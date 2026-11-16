//! Channel layout, sample depth and output-configuration contracts.
//!
//! Resolves channel maps against positioned layouts — a layout with a hole must
//! leave that role empty instead of shifting the next plane into it — and pins
//! the sample-depth and channel-configuration defaults the stream setup reads.
//! No audio device is opened, no playback happens and no database is touched.

use crate::audio::engine::{AudioEngine, BitDepth, ChannelConfig, ChannelMap};

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

/// El mapa de canales debe resolver cada rol según las posiciones realmente
/// presentes en el layout: una posición ausente (el central en 2.1 o el LFE
/// en 5.0, por ejemplo) no debe apuntar al canal que ocupa ese índice de
/// conteo. Antes, `get_channel_map` consultaba las ocho posiciones sin
/// comprobar pertenencia y los layouts con huecos enrutaban el plano
/// equivocado a center/LFE/surround.
#[test]
fn test_channel_map_positioned_layouts_gate_absent_roles() {
    use symphonia::core::audio::layouts::{
        CHANNEL_LAYOUT_2P1, CHANNEL_LAYOUT_3P0_REAR, CHANNEL_LAYOUT_4P0_QUAD_SIDE,
        CHANNEL_LAYOUT_5P0,
    };

    // 2.1 (FL/FR/LFE): el central y los surround no están presentes.
    let map = AudioEngine::get_channel_map(CHANNEL_LAYOUT_2P1);
    assert_eq!(map.fl, Some(0));
    assert_eq!(map.fr, Some(1));
    assert_eq!(map.lfe, Some(2));
    assert_eq!(map.c, None, "2.1 no tiene central");
    assert_eq!(map.sl, None);
    assert_eq!(map.sr, None);
    assert_eq!(map.sbl, None);
    assert_eq!(map.sbr, None);

    // 5.0 (FL/FR/C/RL/RR): el LFE y los side no están presentes.
    let map = AudioEngine::get_channel_map(CHANNEL_LAYOUT_5P0);
    assert_eq!(map.fl, Some(0));
    assert_eq!(map.fr, Some(1));
    assert_eq!(map.c, Some(2));
    assert_eq!(map.sbl, Some(3));
    assert_eq!(map.sbr, Some(4));
    assert_eq!(map.lfe, None, "5.0 no tiene LFE");
    assert_eq!(map.sl, None);
    assert_eq!(map.sr, None);

    // Quad con side (FL/FR/SL/SR): el central y el LFE no están presentes.
    let map = AudioEngine::get_channel_map(CHANNEL_LAYOUT_4P0_QUAD_SIDE);
    assert_eq!(map.fl, Some(0));
    assert_eq!(map.fr, Some(1));
    assert_eq!(map.sl, Some(2));
    assert_eq!(map.sr, Some(3));
    assert_eq!(map.c, None);
    assert_eq!(map.lfe, None);
    assert_eq!(map.sbl, None);
    assert_eq!(map.sbr, None);

    // 3.0 trasero (FL/FR/RC): el central no debe mapearse al rear-center.
    let map = AudioEngine::get_channel_map(CHANNEL_LAYOUT_3P0_REAR);
    assert_eq!(map.fl, Some(0));
    assert_eq!(map.fr, Some(1));
    assert_eq!(map.c, None, "el rear-center no es el central");
    assert_eq!(map.lfe, None);
    assert_eq!(map.sbl, None);
    assert_eq!(map.sbr, None);
}

/// Un layout etiquetado (`Channels::Custom`) resuelve los roles por su
/// etiqueta posicionada exacta; las etiquetas discretas no aportan roles.
#[test]
fn test_channel_map_custom_positioned_labels() {
    use symphonia::core::audio::{ChannelLabel, Channels, Position};

    let labels = vec![
        ChannelLabel::Positioned(Position::FRONT_LEFT),
        ChannelLabel::Positioned(Position::FRONT_RIGHT),
        ChannelLabel::Discrete(0),
        ChannelLabel::Positioned(Position::LFE1),
    ];
    let map = AudioEngine::get_channel_map(Channels::from(labels));
    assert_eq!(map.fl, Some(0));
    assert_eq!(map.fr, Some(1));
    assert_eq!(map.lfe, Some(3));
    assert_eq!(map.c, None);
    assert_eq!(map.sl, None);
    assert_eq!(map.sr, None);
    assert_eq!(map.sbl, None);
    assert_eq!(map.sbr, None);
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
