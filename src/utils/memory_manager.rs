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

fn get_sysinfo() -> &'static parking_lot::Mutex<sysinfo::System> {
    SYSINFO.get_or_init(|| parking_lot::Mutex::new(sysinfo::System::new()))
}

/// Devuelve el porcentaje de RAM usado (0.0 - 100.0)
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

/// Devuelve el intervalo de purga en minutos basado en el uso real de RAM.
/// - RAM < 50%:  purga cada 15 minutos (relajado)
/// - RAM 50-70%: purga cada 8 minutos
/// - RAM 70-85%: purga cada 4 minutos
/// - RAM > 85%:  purga cada 1 minuto (agresivo)
pub fn get_dynamic_purge_interval_mins() -> u64 {
    let ram_pct = get_ram_usage_percent();
    if ram_pct > 85.0 {
        1
    } else if ram_pct > 70.0 {
        4
    } else if ram_pct > 50.0 {
        8
    } else {
        15
    }
}

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

    /// Verifica si han pasado X minutos desde la última purga global.
    /// El intervalo se calcula dinámicamente según el uso real de RAM.
    pub fn should_run_global_purge(_interval_mins: u64, is_scanning: bool) -> bool {
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

        // Usar intervalo dinámico basado en RAM real
        let interval_mins = get_dynamic_purge_interval_mins();

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
        iced::Task::done(crate::gui::app::Message::GlobalMemoryPurge)
    }
}
