//! Latency, format acceptance and device-resolution tests for the device manager.
//!
//! These tests exercise the pure decisions the device manager makes before it asks
//! CPAL for anything: the platform base latency, the sample formats the output
//! callback can convert, and the preference order between a stored device id and a
//! legacy name. No test opens an audio device or builds a stream, so the module can
//! be pinned on any machine and under any host.

use super::*;

#[test]
#[cfg(target_os = "linux")]
fn linux_base_latency_is_10ms() {
    assert_eq!(platform_base_latency_ms(), 10.0);
}

/// `accepted_sample_format` acepta exactamente los formatos que el callback
/// puede convertir y rechaza el resto. `SampleFormat` es `#[non_exhaustive]`,
/// así que las variantes conocidas se enumeran explícitamente.
#[test]
fn test_cpal_format_mapping_exhaustive() {
    let all_formats = [
        cpal::SampleFormat::I8,
        cpal::SampleFormat::I16,
        cpal::SampleFormat::I24,
        cpal::SampleFormat::I32,
        cpal::SampleFormat::I64,
        cpal::SampleFormat::U8,
        cpal::SampleFormat::U16,
        cpal::SampleFormat::U24,
        cpal::SampleFormat::U32,
        cpal::SampleFormat::U64,
        cpal::SampleFormat::F32,
        cpal::SampleFormat::F64,
        cpal::SampleFormat::DsdU8,
        cpal::SampleFormat::DsdU16,
        cpal::SampleFormat::DsdU32,
    ];

    for fmt in all_formats {
        let expected = matches!(
            fmt,
            cpal::SampleFormat::F32
                | cpal::SampleFormat::F64
                | cpal::SampleFormat::I16
                | cpal::SampleFormat::I32
                | cpal::SampleFormat::U16
        );
        assert_eq!(
            accepted_sample_format(fmt),
            expected,
            "mapeo inesperado para {fmt:?}"
        );
    }

    assert!(accepted_sample_format(cpal::SampleFormat::F32));
    assert!(!accepted_sample_format(cpal::SampleFormat::I24));
    assert!(!accepted_sample_format(cpal::SampleFormat::U32));
}

/// La resolución prefiere el id estable sobre el nombre heredado y cae al
/// predeterminado cuando la salida guardada ya no está.
#[test]
fn test_resolve_device_id_prefers_stable() {
    let present = vec![
        ("alsa:one".to_string(), "DAC Uno".to_string()),
        ("alsa:two".to_string(), "DAC Dos".to_string()),
    ];

    // Con id estable y nombre heredado presentes, gana el id.
    assert_eq!(
        choose_device_lookup(Some("alsa:two"), Some("DAC Uno"), &present),
        DeviceLookup::StableId(1)
    );
    // Sin id, cae al nombre heredado.
    assert_eq!(
        choose_device_lookup(None, Some("DAC Uno"), &present),
        DeviceLookup::LegacyName(0)
    );
    // Con un id no presente pero un nombre válido, usa el nombre.
    assert_eq!(
        choose_device_lookup(Some("alsa:missing"), Some("DAC Dos"), &present),
        DeviceLookup::LegacyName(1)
    );
    // Con algo guardado sin coincidencia, avisa del fallback.
    assert_eq!(
        choose_device_lookup(Some("alsa:missing"), None, &present),
        DeviceLookup::Missing
    );
    // Sin nada guardado, usa el predeterminado.
    assert_eq!(
        choose_device_lookup(None, None, &present),
        DeviceLookup::Default
    );
}
