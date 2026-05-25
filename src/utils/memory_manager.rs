use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Gestor global del ciclo de vida y recolección de basura de la memoria (GC Automático).
pub struct MemoryManager;

// Marca de tiempo del inicio de la aplicación y última purga
static START_TIME: AtomicU64 = AtomicU64::new(0);
static LAST_GLOBAL_PURGE: AtomicU64 = AtomicU64::new(0);

/// Cached system info para evitar crear el objeto sysinfo::System en cada consulta
static SYSINFO: OnceLock<parking_lot::Mutex<sysinfo::System>> = OnceLock::new();

/// Intervalo fijo de purga: 120 segundos (2 minutos). D-01: estricto, sin backoff ni lógica adaptativa.
const FIXED_PURGE_INTERVAL_SECS: u64 = 120;

/// Umbral de hard cap de RAM: si el uso supera este porcentaje, se fuerza purga inmediata. D-03.
const RAM_HARD_CAP_PERCENT: f64 = 75.0;

/// Obtiene o inicializa la instancia global de `sysinfo::System`.
fn get_sysinfo() -> &'static parking_lot::Mutex<sysinfo::System> {
    SYSINFO.get_or_init(|| parking_lot::Mutex::new(sysinfo::System::new()))
}

/// Devuelve el porcentaje de RAM usado (0.0 - 100.0).
///
/// Usa `sysinfo` para leer memoria total y usada del sistema.
fn get_ram_usage_percent() -> f64 {
    if let Some(mut sys) = get_sysinfo().try_lock() {
        sys.refresh_memory();
        let total = sys.total_memory();
        let used = sys.used_memory();
        if total > 0 {
            return (used as f64 / total as f64) * 100.0;
        }
    }
    0.0
}

impl MemoryManager {
    /// Inicializa los temporizadores internos de purga.
    ///
    /// Debe llamarse una sola vez al arrancar la aplicación.
    pub fn init() {
        let now = Self::get_now_secs();
        START_TIME.store(now, Ordering::Relaxed);
        LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
    }

    /// Registra actividad general (actualmente no operativa).
    pub fn register_activity() {}
    /// Registra actividad en la playlist (actualmente no operativa).
    pub fn register_playlist_activity() {}
    /// Registra actividad en la biblioteca (actualmente no operativa).
    pub fn register_library_activity() {}

    /// Reinicia el temporizador de la purga global (útil tras escaneos o acciones masivas)
    pub fn reset_global_purge_timer() {
        let now = Self::get_now_secs();
        LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
    }

    /// Verifica si han pasado 120 segundos desde la última purga global.
    /// Usa un intervalo fijo de 2 minutos (D-01), sin lógica adaptativa.
    /// No se activa durante escaneos.
    pub fn should_run_global_purge(_ignored: u64, is_scanning: bool) -> bool {
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

        if now.saturating_sub(last) >= FIXED_PURGE_INTERVAL_SECS {
            LAST_GLOBAL_PURGE.store(now, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Devuelve la marca de tiempo UNIX actual en segundos.
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

    /// Comprueba si la RAM del sistema supera el hard cap (75%). D-03.
    /// Si retorna `true`, el Tick handler debe forzar un `GlobalMemoryPurge` inmediato
    /// sin esperar el ciclo de 2 minutos.
    pub fn is_ram_over_hard_cap() -> bool {
        let ram_pct = get_ram_usage_percent();
        ram_pct > RAM_HARD_CAP_PERCENT
    }

    /// Ejecuta la secuencia de purga global notificando a la App
    pub fn execute_global_purge() -> iced::Task<crate::gui::app::Message> {
        iced::Task::done(crate::gui::app::Message::GlobalMemoryPurge)
    }
}
