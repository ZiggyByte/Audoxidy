//! Display, source and conversion tests for the audio error type.
//!
//! Every message pinned here reaches the user interface or a log line, so the tests
//! hold the exact text of each variant and the presence of a source only for the I/O
//! variant. The type is inert: nothing in this module opens a device or starts a thread.

use super::*;
use std::error::Error as _;

#[test]
fn display_messages_are_byte_identical() {
    assert_eq!(AudioError::NoDevice.to_string(), "No audio device found");
    assert_eq!(
        AudioError::NoActiveOutput.to_string(),
        "No hay una salida de audio activa para purgar"
    );
    assert_eq!(AudioError::HostNotFound.to_string(), "Host no encontrado");
    assert_eq!(
        AudioError::DeviceNotFound.to_string(),
        "Dispositivo no encontrado"
    );
    assert_eq!(
        AudioError::UnsupportedSampleFormat {
            device: "DAC".into(),
            format: "I24".into(),
        }
        .to_string(),
        "Formato de muestra no soportado por el dispositivo 'DAC': I24"
    );
    assert_eq!(
        AudioError::UnsupportedFormat("FLAC corrupto".into()).to_string(),
        "Formato de audio no soportado: FLAC corrupto"
    );
    assert_eq!(
        AudioError::ConfigError("x".into()).to_string(),
        "Config error: x"
    );
    assert_eq!(
        AudioError::StreamError("x".into()).to_string(),
        "Stream error: x"
    );
    assert_eq!(
        AudioError::DeviceError("x".into()).to_string(),
        "Device error: x"
    );
    assert_eq!(
        AudioError::DecodeError("x".into()).to_string(),
        "Decode error: x"
    );
    assert_eq!(
        AudioError::IoError(std::io::Error::other("boom")).to_string(),
        "I/O error: boom"
    );
}

#[test]
fn source_is_present_only_for_io_error() {
    assert!(
        AudioError::IoError(std::io::Error::other("boom"))
            .source()
            .is_some()
    );
    assert!(AudioError::NoDevice.source().is_none());
}

#[test]
fn from_io_error_converts() {
    let error: AudioError = std::io::Error::other("x").into();
    assert!(matches!(error, AudioError::IoError(_)));
}
