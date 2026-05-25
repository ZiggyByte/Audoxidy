//! Módulo de audio de Audoxidy.
//!
//! Contiene el motor de reproducción, cadena DSP, gestión de dispositivos,
//! decodificación de formatos y ecualización por presets.

pub mod decoder;
pub mod device_manager;
pub mod dsp;
pub mod engine;
pub mod error;
pub mod manager;
pub mod preset;
#[cfg(test)]
pub mod tests;
#[cfg(test)]
pub mod integration_tests;

pub use error::AudioError;
pub use manager::AudioManager;
