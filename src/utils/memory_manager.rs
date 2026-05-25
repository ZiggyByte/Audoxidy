use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Gestor global del ciclo de vida y recolección de basura de la memoria (GC Automático).
pub struct MemoryManager;

// Marca de tiempo del inicio de la aplicación y última purga
static START_TIME: AtomicU64 = AtomicU64::new(0);
static LAST_GLOBAL_PURGE: AtomicU64 = AtomicU64::new(0);

impl MemoryManager {
    /// Inicializa los temporizadores al arrancar la aplicación
    pub fn init() {
        let now = Self::get_now_secs();
        START_TIME.store(now, Ordering::Relaxed);
        LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
    }

    pub fn register_activity() {}
    pub fn register_playlist_activity() {}
    pub fn register_library_activity() {}

    /// Reinicia el temporizador de la purga global (útil tras escaneos o acciones masivas)
    pub fn reset_global_purge_timer() {
        let now = Self::get_now_secs();
        LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
    }

    /// Verifica si han pasado X minutos desde la última purga global
    pub fn should_run_global_purge(interval_mins: u64, is_scanning: bool) -> bool {
        if is_scanning {
            return false;
        }

        let now = Self::get_now_secs();
        let last = LAST_GLOBAL_PURGE.load(Ordering::Relaxed);

        // Si es 0 (primera ejecución), inicializamos
        if last == 0 {
            LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
            return false;
        }

        if now.saturating_sub(last) >= (interval_mins * 60) {
            LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
            return true;
        }
        false
    }

    fn get_now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Fuerza al asignador de memoria a devolver la RAM libre al Sistema Operativo (Linux)
    pub fn force_free_to_os() {
        #[cfg(target_os = "linux")]
        unsafe {
            unsafe extern "C" {
                fn malloc_trim(pad: usize) -> i32;
            }
            malloc_trim(0);
        }
    }

    /// Ejecuta la secuencia de purga global notificando a la App
    pub fn execute_global_purge() -> iced::Task<crate::gui::app::Message> {
        // Simplemente enviamos el mensaje a la App para que ella orqueste la limpieza secuencial
        iced::Task::done(crate::gui::app::Message::GlobalMemoryPurge)
    }
}
