//! Persistence logging shared by the application shell.

/// Registra en el log un fallo al persistir un ajuste, indicando la clave afectada.
pub(in crate::gui::app) fn log_persist(key: &str, result: rusqlite::Result<()>) {
    if let Err(e) = result {
        tracing::warn!(
            target: "audoxidy::persistence",
            key,
            error = %e,
            "No se pudo persistir el ajuste"
        );
    }
}
