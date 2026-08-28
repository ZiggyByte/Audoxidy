use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Gestor global del ciclo de vida y recolección de basura de la memoria (GC Automático).
pub struct MemoryManager;

// Marca de tiempo del inicio de la aplicación y última purga
static START_TIME: AtomicU64 = AtomicU64::new(0);
static LAST_GLOBAL_PURGE: AtomicU64 = AtomicU64::new(0);

// Última purga forzada por exceso de RAM (cooldown: evita purgar en cada tick).
static LAST_RAM_PURGE: AtomicU64 = AtomicU64::new(0);

/// Cooldown mínimo entre purgas forzadas por RAM (10s): sin él, con la RAM por
/// encima del umbral la purga se dispararía en cada tick (500ms) y entraría en
/// bucle con la re-creación de la pre-carga (log "GC: Purging" repetido).
const RAM_PURGE_COOLDOWN_SECS: u64 = 10;

/// Cached system info para evitar crear el objeto sysinfo::System en cada consulta
static SYSINFO: OnceLock<parking_lot::Mutex<sysinfo::System>> = OnceLock::new();

/// Intervalo fijo de purga: 60 segundos (1 minuto). D-01: estricto, sin backoff ni lógica adaptativa.
const FIXED_PURGE_INTERVAL_SECS: u64 = 60;

/// Umbral de hard cap de RAM: si el uso supera este porcentaje, se fuerza purga inmediata. D-03.
const RAM_HARD_CAP_PERCENT: f64 = 75.0;

/// Tope de RAM del propio reproductor: si supera este valor (MB), se fuerza purga
/// inmediata aunque el sistema no esté al límite.
const APP_RAM_MAX_MB: u64 = 500;

/// Tamaño de página del sistema (Linux: 4096 bytes típicamente).
const PAGE_SIZE_BYTES: u64 = 4096;

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
    /// sin esperar el ciclo de purga periódico.
    pub fn is_ram_over_hard_cap() -> bool {
        let ram_pct = get_ram_usage_percent();
        ram_pct > RAM_HARD_CAP_PERCENT
    }

    /// Decide si debe forzarse una purga por exceso de RAM, con COOLDOWN de 10s.
    ///
    /// Sin el cooldown, con la RAM por encima del umbral la purga se dispararía en
    /// cada tick (500ms), y al purgar la pre-carga (que se re-crea al instante) el
    /// bucle GC → pre-carga → GC se repetiría indefinidamente (log "GC: Purging"
    /// cada pocos cientos de ms).
    pub fn should_purge_for_ram() -> bool {
        if !(Self::is_app_ram_over_limit() || Self::is_ram_over_hard_cap()) {
            return false;
        }
        let now = Self::get_now_secs();
        let last = LAST_RAM_PURGE.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= RAM_PURGE_COOLDOWN_SECS {
            LAST_RAM_PURGE.store(now, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Devuelve la RAM física usada por este proceso en MB (Linux: /proc/self/statm).
    fn get_self_ram_mb() -> u64 {
        #[cfg(target_os = "linux")]
        {
            use std::io::Read;
            if let Ok(mut f) = std::fs::File::open("/proc/self/statm") {
                let mut buf = String::new();
                if f.read_to_string(&mut buf).is_ok() {
                    // statm: size resident shared text lib data dt (páginas).
                    if let Some(rss_pages) = buf.split_whitespace().nth(1) {
                        if let Ok(pages) = rss_pages.parse::<u64>() {
                            return (pages * PAGE_SIZE_BYTES) / (1024 * 1024);
                        }
                    }
                }
            }
        }
        0
    }

    /// Comprueba si la RAM del propio reproductor supera el tope (500 MB).
    /// Usada por el Tick para forzar purgas tempranas del proceso (no solo del sistema).
    pub fn is_app_ram_over_limit() -> bool {
        let mb = Self::get_self_ram_mb();
        mb > APP_RAM_MAX_MB
    }

    /// Ejecuta la secuencia de purga global notificando a la App
    pub fn execute_global_purge() -> iced::Task<crate::gui::app::Message> {
        iced::Task::done(crate::gui::app::Message::GlobalMemoryPurge)
    }
}
