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

/// Intervalo fijo de purga: 60 segundos (1 minuto); estricto, sin backoff ni lógica adaptativa.
const FIXED_PURGE_INTERVAL_SECS: u64 = 60;

/// Umbral de hard cap de RAM: si el uso supera este porcentaje, se fuerza purga inmediata.
const RAM_HARD_CAP_PERCENT: f64 = 75.0;

/// Tope de RAM del propio reproductor: si supera este valor (MB), se fuerza purga
/// inmediata aunque el sistema no esté al límite.
const APP_RAM_MAX_MB: u64 = 500;

/// Obtiene o inicializa la instancia global de `sysinfo::System`.
fn get_sysinfo() -> &'static parking_lot::Mutex<sysinfo::System> {
    SYSINFO.get_or_init(|| parking_lot::Mutex::new(sysinfo::System::new()))
}

/// Toma el lock global de `sysinfo` con reintentos acotados.
///
/// Bajo contención (tests en paralelo o el hilo de GC leyendo a la vez) un único
/// `try_lock` puede fallar y devolver valores 0 espurios. Reintentar durante un
/// intervalo muy corto evita esa falsa lectura sin bloquear indefinidamente al
/// llamante: tras agotar los intentos devuelve `None` y la lectura cae a 0.
fn try_lock_sysinfo() -> Option<parking_lot::MutexGuard<'static, sysinfo::System>> {
    const MAX_ATTEMPTS: u32 = 100;
    let mut attempts = 0;
    loop {
        if let Some(guard) = get_sysinfo().try_lock() {
            return Some(guard);
        }
        attempts += 1;
        if attempts >= MAX_ATTEMPTS {
            return None;
        }
        std::thread::yield_now();
    }
}

/// Devuelve la memoria del sistema en bytes como `(usada, total)`.
///
/// Usa `sysinfo`; `(0, 0)` si el sistema no expone los valores o si el lock
/// permanece ocupado tras los reintentos.
fn get_system_ram_bytes() -> (u64, u64) {
    if let Some(mut sys) = try_lock_sysinfo() {
        sys.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        return (sys.used_memory(), sys.total_memory());
    }
    (0, 0)
}

/// Comprueba si el uso de RAM del sistema supera el hard cap.
///
/// Es estricto: un uso exactamente igual al umbral NO cuenta como superado, y un
/// total desconocido (0) nunca se considera por encima.
pub(crate) fn system_ram_over_hard_cap(used: u64, total: u64) -> bool {
    total > 0 && (used as f64 / total as f64) * 100.0 > RAM_HARD_CAP_PERCENT
}

/// Comprueba si la RAM del propio proceso supera el tope configurado.
///
/// Es estricto: un uso exactamente igual al tope NO cuenta como superado.
pub(crate) fn process_ram_over_limit(process_mb: u64) -> bool {
    process_mb > APP_RAM_MAX_MB
}

/// Decide si procede una purga forzada por RAM y devuelve la marca de tiempo a registrar.
///
/// Devuelve `None` si no se supera el umbral o si todavía no ha transcurrido el
/// cooldown desde la última purga; `Some(now_secs)` cuando debe purgarse ahora.
pub(crate) fn ram_purge_decision(
    over_cap: bool,
    now_secs: u64,
    last_purge_secs: u64,
) -> Option<u64> {
    if !over_cap {
        return None;
    }
    if now_secs.saturating_sub(last_purge_secs) >= RAM_PURGE_COOLDOWN_SECS {
        Some(now_secs)
    } else {
        None
    }
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
    /// Usa un intervalo fijo de 2 minutos, sin lógica adaptativa.
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

    /// Comprueba si la RAM del sistema supera el hard cap (75%).
    /// Si retorna `true`, el Tick handler debe forzar un `GlobalMemoryPurge` inmediato
    /// sin esperar el ciclo de purga periódico.
    pub fn is_ram_over_hard_cap() -> bool {
        let (used, total) = get_system_ram_bytes();
        system_ram_over_hard_cap(used, total)
    }

    /// Decide si debe forzarse una purga por exceso de RAM, con COOLDOWN de 10s.
    ///
    /// Sin el cooldown, con la RAM por encima del umbral la purga se dispararía en
    /// cada tick (500ms), y al purgar la pre-carga (que se re-crea al instante) el
    /// bucle GC → pre-carga → GC se repetiría indefinidamente (log "GC: Purging"
    /// cada pocos cientos de ms).
    pub fn should_purge_for_ram() -> bool {
        let over = Self::is_app_ram_over_limit() || Self::is_ram_over_hard_cap();
        let now = Self::get_now_secs();
        let last = LAST_RAM_PURGE.load(Ordering::Relaxed);
        match ram_purge_decision(over, now, last) {
            Some(accepted_at) => {
                LAST_RAM_PURGE.store(accepted_at, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// Devuelve la RAM física usada por este proceso en MB.
    ///
    /// Lee el RSS vía `sysinfo` (multiplataforma: Linux, macOS y Windows), reutilizando
    /// el `System` cacheado y refrescando únicamente el proceso actual. Sin `unwrap`:
    /// si no se puede resolver el PID o el proceso, devuelve 0 para no romper el Tick.
    fn get_self_ram_mb() -> u64 {
        if let Some(mut sys) = try_lock_sysinfo() {
            if let Ok(pid) = sysinfo::get_current_pid() {
                sys.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[pid]),
                    false,
                    sysinfo::ProcessRefreshKind::nothing().with_memory(),
                );
                if let Some(process) = sys.process(pid) {
                    // Process::memory() devuelve el RSS en bytes; se conserva la
                    // semántica entera en MB que usan el tope de 500 MB y la métrica.
                    return process.memory() / (1024 * 1024);
                }
            }
        }
        0
    }

    /// RAM física usada por este proceso en bytes; fuente para la métrica `ram_usage_bytes`.
    pub fn current_process_ram_bytes() -> u64 {
        Self::get_self_ram_mb() * 1024 * 1024
    }

    /// Comprueba si la RAM del propio reproductor supera el tope (500 MB).
    /// Usada por el Tick para forzar purgas tempranas del proceso (no solo del sistema).
    pub fn is_app_ram_over_limit() -> bool {
        process_ram_over_limit(Self::get_self_ram_mb())
    }

    /// Ejecuta la secuencia de purga global notificando a la App
    pub fn execute_global_purge() -> iced::Task<crate::gui::app::Message> {
        iced::Task::done(crate::gui::app::Message::GlobalMemoryPurge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serializa las pruebas que leen el `SYSINFO` global mediante `try_lock`: si dos
    // hilos compiten por él, `try_lock` falla y la lectura devuelve (0, 0), lo que
    // hacía fallar estas aserciones de forma intermitente bajo `cargo test` paralelo.
    static SYSINFO_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn hard_cap_boundary() {
        // Exactamente el 75% NO supera el hard cap (estricto).
        assert!(!system_ram_over_hard_cap(75, 100));
        // 75.000001% sí lo supera.
        assert!(system_ram_over_hard_cap(75_000_001, 100_000_000));
        // Un total desconocido (0) nunca se considera por encima.
        assert!(!system_ram_over_hard_cap(1, 0));
    }

    #[test]
    fn process_cap_boundary() {
        // Exactamente el tope (500 MB) NO supera el límite (estricto).
        assert!(!process_ram_over_limit(500));
        assert!(process_ram_over_limit(501));
        assert!(!process_ram_over_limit(0));
    }

    #[test]
    fn purge_cooldown() {
        // Sin exceso de RAM nunca se purga.
        assert_eq!(ram_purge_decision(false, 1_000, 0), None);
        // Con exceso, el cooldown de 10s es inclusivo en el límite.
        assert_eq!(ram_purge_decision(true, 1_000, 1_000), None);
        assert_eq!(ram_purge_decision(true, 1_009, 1_000), None);
        assert_eq!(ram_purge_decision(true, 1_010, 1_000), Some(1_010));
        assert_eq!(ram_purge_decision(true, 1_011, 1_000), Some(1_011));
    }

    #[test]
    fn self_rss_probe_returns_nonzero() {
        let _guard = SYSINFO_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ejercita la ruta real de `sysinfo` en la plataforma en la que corre el test;
        // un proceso en ejecución siempre tiene RSS > 0 si el sondeo funciona.
        assert!(
            MemoryManager::get_self_ram_mb() > 0,
            "el sondeo de RSS vía sysinfo devolvió 0"
        );
    }

    #[test]
    fn system_ram_is_sane() {
        let _guard = SYSINFO_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Ejercita la ruta real de lectura de RAM del sistema: un refresh que omitiera
        // `with_ram()` devolvería total = 0.
        let (used, total) = get_system_ram_bytes();
        assert!(total > 0, "sysinfo no reportó RAM total");
        assert!(used <= total, "RAM usada > total");
    }
}
