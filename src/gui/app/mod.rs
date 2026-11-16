//! Application shell module root.
//!
//! Holds the module declarations and re-exports plus the shared seek predicate.
//! The view composition (with its widget adapter) lives in `view`, the update
//! dispatcher in `update`, and the shell tests in the `app_tests` area file.

mod init;
mod message;
mod persistence;
mod state;
mod subscription;
pub(in crate::gui::app) mod tick;
mod update;
mod view;

pub use message::{ActiveDialog, AppFocus, ExportFormat, ExportMode, Message};
pub use state::AudoxidyApp;

/// Indica si la búsqueda fina está habilitada para una duración dada.
///
/// La búsqueda fina solo tiene sentido cuando se conoce la duración de la
/// pista: si la duración no puede derivarse (`0.0`), la posición queda
/// indeterminada y cualquier salto debe ignorarse. Un valor negativo o no
/// finito (`NaN`) también devuelve `false`, porque la comparación es falsa.
pub(crate) fn fine_seek_enabled(total_duration: f64) -> bool {
    total_duration.is_finite() && total_duration > 0.0
}

#[cfg(test)]
mod app_tests;
