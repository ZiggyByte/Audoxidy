//! Audio center tabs module root.
//!
//! Declares the tab content modules and re-exports the panel view.

mod config;
mod effects;
mod equalizer;
mod view;
mod volumen;

pub use view::view;
