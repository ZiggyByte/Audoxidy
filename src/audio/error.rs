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
    /// El archivo no se pudo abrir o decodificar: el probe no reconoce el
    /// contenedor o el códec, o faltan la pista/parámetros de audio. Se nombra
    /// el motivo para que el jugador muestre el aviso en vez de fallar en
    /// silencio.
    #[error("Formato de audio no soportado: {0}")]
    UnsupportedFormat(String),
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
#[path = "error_tests.rs"]
mod tests;
