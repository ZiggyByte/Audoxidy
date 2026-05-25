use std::fmt;

/// Errores del motor de audio de Audoxidy.
///
/// Abarca problemas de dispositivo, stream, configuración, decodificación y E/S.
#[derive(Debug)]
pub enum AudioError {
    NoDevice,
    NoActiveOutput,
    HostNotFound,
    DeviceNotFound,
    UnsupportedSampleFormat,
    ConfigError(String),
    StreamError(String),
    DeviceError(String),
    DecodeError(String),
    IoError(std::io::Error),
}

/// Convierte un `std::io::Error` en `AudioError::IoError`.
impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        AudioError::IoError(e)
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::NoDevice => write!(f, "No audio device found"),
            AudioError::NoActiveOutput => {
                write!(f, "No hay una salida de audio activa para purgar")
            }
            AudioError::HostNotFound => write!(f, "Host no encontrado"),
            AudioError::DeviceNotFound => write!(f, "Dispositivo no encontrado"),
            AudioError::UnsupportedSampleFormat => write!(f, "Formato de muestra no soportado"),
            AudioError::ConfigError(msg) => write!(f, "Config error: {msg}"),
            AudioError::StreamError(msg) => write!(f, "Stream error: {msg}"),
            AudioError::DeviceError(msg) => write!(f, "Device error: {msg}"),
            AudioError::DecodeError(msg) => write!(f, "Decode error: {msg}"),
            AudioError::IoError(e) => write!(f, "I/O error: {e}"),
        }
    }
}

/// Implementación del trait `Error` para compatibilidad con la biblioteca estándar.
impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AudioError::IoError(e) => Some(e),
            _ => None,
        }
    }
}
