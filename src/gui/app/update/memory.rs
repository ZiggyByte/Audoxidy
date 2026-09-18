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
                let is_playing = self.audio_manager.is_playing();
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

                // 4. Purgar carátulas LRU (Purga suave de 16 elementos)
                crate::utils::covers::purge_old_covers(16);

                // 5. Liberar la memoria de la pre-carga de audio (seguro durante la
                //    reproducción: no reinicia el stream). NO se resetea
                //    preloaded_next_path: si se reseteara, la pre-carga se re-crearía
                //    en el siguiente tick (a ~15s del final) y, con la RAM aún por
                //    encima del umbral, el GC → pre-carga → GC entraría en bucle.
                let _ = self.audio_manager.purge_preload();

                // 6. Purga de buffers de audio solo si no hay música sonando
                if !is_playing {
                    let _ = self.audio_manager.purge_buffers();
                }

                // 7. FINAL: Reclamar memoria física al SO (Linux)
                crate::utils::memory_manager::MemoryManager::force_free_to_os();

                Task::none()
            }
            _ => Task::none(),
        }
    }
}
