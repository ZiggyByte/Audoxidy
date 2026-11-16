//! Módulo de audio de Audoxidy.
//!
//! Contiene el motor de reproducción, cadena DSP, gestión de dispositivos,
//! decodificación de formatos y ecualización por presets.

#[cfg(test)]
mod channel_tests;
pub mod decoder;
pub mod device_manager;
pub mod dsp;
pub mod engine;
pub mod error;
#[cfg(test)]
mod fade_tests;
#[cfg(test)]
pub mod integration_tests;
mod loudness;
pub mod manager;
pub mod meter;
#[cfg(test)]
mod mixer_tests;
pub mod preset;
#[cfg(test)]
mod state_tests;

pub use error::AudioError;
pub use manager::AudioManager;
