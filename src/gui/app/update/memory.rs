//! Global memory purge handler extracted from the update dispatcher.

use crate::gui::app::{AppFocus, Message};
use iced::Task;

/// Ventana de seguridad (segundos) tras el fin de la pre-carga durante la cual
/// el GC global no corre: la transición de canciones aún está consumiendo el
/// audio pre-cargado (drenado + mezcla cruzada), y una purga la interrumpiría.
pub(in crate::gui::app) const PRELOAD_GC_SAFETY_SECS: u64 = 20;

/// Indica si el GC global debe quedar bloqueado por la pre-carga: activa o
/// dentro de la ventana de seguridad posterior a su fin.
pub(in crate::gui::app) fn gc_blocked_by_preload(
    now_secs: u64,
    preload_active: bool,
    preload_finished_at: Option<u64>,
) -> bool {
    preload_active
        || preload_finished_at
            .map(|t| now_secs.saturating_sub(t) < PRELOAD_GC_SAFETY_SECS)
            .unwrap_or(false)
}

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_memory(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::GlobalMemoryPurge => {
                tracing::debug!("Audoxidy GC: Purging Memory (Ghost Mode)");

                // 1. Unload de recursos pesados con márgenes de seguridad
                let is_library_focused = self.focus == AppFocus::Library;
                let is_playlist_focused = self.focus == AppFocus::Playlist;
                let is_playing = self.core.is_playing();
                let is_loaded_in_player = self.playlist_manager.playing_song_idx.is_some();

                // Descargar listas inactivas (la pestaña activa siempre se conserva)
                self.playlist_manager.unload(
                    is_playlist_focused,
                    is_playing || is_loaded_in_player,
                    true,
                );

                // Descargar biblioteca inactiva
                self.library_manager.unload(is_library_focused);

                // 2. Limpieza de cachés de virtualización UI (Siempre se limpian para liberar RAM, se regeneran al vuelo)
                self.library_manager.invalidate_cache();
                self.library_manager.invalidate_visible_cache();
                self.playlist_manager.invalidate_cache();

                // Solo limpiar filtros si no estamos en la biblioteca para evitar que desaparezcan visualmente
                if !is_library_focused {
                    self.filters_manager.invalidate_cache();
                }

                // 3. Limpiar caché global de strings (Interner)
                crate::utils::interner::clear_interner();

                // 4. Release the decoded covers the current window does not show;
                //    the visible ones are kept, so a purge during playback cannot
                //    blank a cover.
                self.library_manager.cover_store.trim_outside_window();

                // 5. Liberar la memoria de la pre-carga de audio (seguro durante la
                //    reproducción: no reinicia el stream). NO se resetea
                //    preloaded_next_path: si se reseteara, la pre-carga se re-crearía
                //    en el siguiente tick (a ~15s del final) y, con la RAM aún por
                //    encima del umbral, el GC → pre-carga → GC entraría en bucle.
                let _ = self.core.purge_preload();

                // 6. Purga de buffers de audio solo si no hay música sonando
                if !is_playing {
                    let _ = self.core.purge_buffers();
                }

                // 7. FINAL: Reclamar memoria física al SO (Linux)
                crate::core::Core::execute_global_purge();

                // 8. Sweep the cover files no row references any more. The task
                //    runs on a worker and reads both guards there, so a scan or
                //    a cover write that starts after this dispatch still keeps
                //    the sweep away.
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let scanning = core.is_scanning();
                        let cover_work = crate::gui::library::cover_cache::cover_work_in_flight()
                            + crate::utils::covers::cover_writes_in_flight() as u64;
                        core.sweep_orphan_covers(scanning, cover_work > 0)
                    },
                    |result| match result {
                        Ok(report) => Message::CoverSweepFinished {
                            removed: report.removed,
                            failed: report.failed,
                        },
                        Err(_) => Message::NoOp,
                    },
                )
            }
            _ => Task::none(),
        }
    }
}
