
#[cfg(test)]
mod tests {
    use crate::audio::engine::{AudioEngine, ChannelMap};

    #[test]
    fn test_mix_channels_planar_stereo_to_stereo() {
        // Setup simple 2.0 -> 2.0 mix
        let frames = 4;
        let in_channels = 2;
        let out_channels = 2;
        
        let input = vec![
            vec![1.0, 1.0, 1.0, 1.0], // Left
            vec![0.5, 0.5, 0.5, 0.5], // Right
        ];
        
        // Manual Map setup (mocking internal logic)
        let map = ChannelMap {
            fl: Some(0), fr: Some(1),
            c: None, lfe: None,
            sbl: None, sbr: None,
            sl: None, sr: None,
            ..Default::default()
        };

        let result = AudioEngine::mix_channels_planar(&input, frames, in_channels, out_channels, &map, (1.0, 1.0, 1.0, 1.0));

        assert_eq!(result.len(), frames * out_channels);
        // Interleaved result expected: L, R, L, R...
        assert_eq!(result[0], 1.0);
        assert_eq!(result[1], 0.5);
        assert_eq!(result[2], 1.0);
        assert_eq!(result[3], 0.5);
    }

    #[test]
    fn test_mix_channels_planar_stereo_to_5_1_simple() {
        // 2.0 -> 5.1 (FL, FR map to FL, FR. Others silent/upmixed depending on logic)
        let frames = 2;
        let in_channels = 2;
        let out_channels = 6;
        
        let input = vec![
            vec![1.0, 1.0],
            vec![0.5, 0.5],
        ];
        
        // Map where Source FL -> Target FL (0), Source FR -> Target FR (1)
        // FC (2), LFE (3), BL (4), BR (5)
        let mut map = ChannelMap::default();
        map.fl = Some(0);
        map.fr = Some(1);
        
        // Our mixing logic might do matrix upmixing if map entries are missing for source, 
        // but here the map defines where SOURCE channels go.
        // Wait, ChannelMap struct definition in engine.rs:
        // struct ChannelMap { fl: Option<usize>, ... } 
        // implies "Target channel index for Source FL".
        
        let result = AudioEngine::mix_channels_planar(&input, frames, in_channels, out_channels, &map, (1.0, 1.0, 1.0, 1.0));
        
        assert_eq!(result.len(), frames * out_channels);
        
        // Frame 0
        assert_eq!(result[0], 1.0); // FL
        assert_eq!(result[1], 0.5); // FR
        assert_eq!(result[2], 0.0); // FC
        assert_eq!(result[3], 0.0); // LFE
        assert_eq!(result[4], 0.0); // BL
        assert_eq!(result[5], 0.0); // BR
    }
}
