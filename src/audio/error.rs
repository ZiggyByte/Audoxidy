use thiserror::Error;

/// Errores del motor de audio de Audoxidy.
///
/// Abarca problemas de dispositivo, stream, configuración, decodificación y E/S.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("No audio device found")]
    NoDevice,
    #[error("No hay una salida de audio activa para purgar")]
    NoActiveOutput,
    #[error("Host no encontrado")]
    HostNotFound,
    #[error("Dispositivo no encontrado")]
    DeviceNotFound,
    /// El dispositivo de salida no ofrece ningún formato de muestra que el
    /// motor pueda convertir; nombra el dispositivo y el formato rechazado
    /// para que la interfaz muestre un aviso en vez de un silencio.
    #[error("Formato de muestra no soportado por el dispositivo '{device}': {format}")]
    UnsupportedSampleFormat { device: String, format: String },
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Stream error: {0}")]
    StreamError(String),
    #[error("Device error: {0}")]
    DeviceError(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
    /// Envuelve un fallo de E/S; `#[from]` genera `From<std::io::Error>` y expone la
    /// causa subyacente a través de `source()`.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
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
        let e: AudioError = std::io::Error::other("x").into();
        assert!(matches!(e, AudioError::IoError(_)));
    }
}
