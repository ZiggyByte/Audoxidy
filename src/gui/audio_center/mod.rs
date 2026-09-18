//! Advanced audio settings panel: device configuration, equalizer, effects,
//! and volume/mix.
//!
//! The module root re-exports the manager, its message vocabulary, and the
//! panel `view` so existing `crate::gui::audio_center::*` paths keep
//! resolving unchanged.

mod messages;
mod state;
mod tabs;
mod update;

pub use messages::{AudioCenterMessage, AudioStateToggle, DspEffect};
pub use messages::{EqStateBackup, SystemSelection};
pub use state::AudioCenterManager;
pub use tabs::view;
