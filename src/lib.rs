pub mod audio;
pub mod db;
pub mod gui;
pub mod integrations;
pub mod utils;

#[cfg(test)]
#[path = "audio/dsp_tests.rs"]
mod dsp_tests;

#[cfg(test)]
#[path = "gui/widgets_tests.rs"]
mod widgets_tests;
