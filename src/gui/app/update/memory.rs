//! Global memory purge handler extracted from the update dispatcher.

use crate::gui::app::{AppFocus, Message};
use crate::utils::observability::COVERS_TARGET;
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

                // Descargar listas inactivas. La pestaña activa ya está cubierta
                // por `is_playlist_focused`; el tercer argumento ya no fuerza `true`,
                // que era lo que impedía toda descarga.
                self.playlist_manager.unload(
                    is_playlist_focused,
                    is_playing || is_loaded_in_player,
                    false,
                );

                // La biblioteca no se descarga: sus datos en RAM son el modelo que la
                // vista necesita para dibujarse, y recortarlos obligaba a releer la
                // base y a reconstruir las agrupaciones, lo que vaciaba las listas
                // hasta el siguiente cambio de vista. La memoria de carátulas la
                // acota el presupuesto de `cover_store`, que este handler recorta
                // más abajo solo cuando la biblioteca no tiene el foco.

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

                // With the library out of focus the prefetch band and the scroll-back
                // cushion are dead weight; the window the view is showing is kept, so
                // returning to the same scroll position is still instant. A scroll
                // into the trimmed band re-decodes those covers asynchronously.
                if !is_library_focused {
                    let dropped = self.library_manager.cover_store.trim_outside_window();
                    tracing::debug!(
                        target: COVERS_TARGET,
                        dropped,
                        retained_bytes = self.library_manager.cover_store.bytes(),
                        "GC: carátulas decodificadas fuera de la ventana liberadas"
                    );
                }

                // 4. Liberar la memoria de la pre-carga de audio (seguro durante la
                //    reproducción: no reinicia el stream). NO se resetea
                //    preloaded_next_path: si se reseteara, la pre-carga se re-crearía
                //    en el siguiente tick (a ~15s del final) y, con la RAM aún por
                //    encima del umbral, el GC → pre-carga → GC entraría en bucle.
                let _ = self.core.purge_preload();

                // 5. Purga de buffers de audio solo si no hay música sonando
                if !is_playing {
                    let _ = self.core.purge_buffers();
                }

                // 6. FINAL: Reclamar memoria física al SO (Linux)
                crate::core::Core::execute_global_purge();

                // 7. Sweep the cover files no row references any more. The task
                //    runs on a worker and reads both guards there, so a scan or
                //    a cover write that starts after this dispatch still keeps
                //    the sweep away. A decode is read-only and cannot create an
                //    unreferenced file, so it is not a guard: counting it here
                //    would skip the sweep of a scan whose covers are still
                //    decoding.
                let core = self.core.clone();
                Task::perform(
                    async move {
                        let scanning = core.is_scanning();
                        let cover_writes = crate::utils::covers::cover_writes_in_flight() > 0;
                        core.sweep_orphan_covers(scanning, cover_writes)
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
