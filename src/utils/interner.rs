use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Máximo de bytes acumulados en la caché antes de purgar (~10 MB)
const INTERNER_MEMORY_LIMIT: u64 = 10 * 1024 * 1024;

static STRING_CACHE: OnceLock<Mutex<HashSet<Arc<str>>>> = OnceLock::new();
static TOTAL_ALLOCATED: AtomicU64 = AtomicU64::new(0);

fn get_cache() -> &'static Mutex<HashSet<Arc<str>>> {
    STRING_CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn check_and_purge() {
    if TOTAL_ALLOCATED.load(Ordering::Relaxed) > INTERNER_MEMORY_LIMIT {
        clear_interner();
    }
}

/// Devuelve un puntero Arc<str> a la cadena proporcionada.
/// Optimizado para guardar la cadena una sola vez en memoria (HashSet de Arcs).
/// Auto-purga cuando la caché excede ~10 MB para prevenir OOM.
pub fn intern_string(s: &str) -> std::sync::Arc<str> {
    let mut cache = get_cache().lock().unwrap();
    if let Some(existing) = cache.get(s) {
        return Arc::clone(existing);
    }
    let arc: Arc<str> = Arc::from(s);
    TOTAL_ALLOCATED.fetch_add(s.len() as u64, Ordering::Relaxed);
    cache.insert(Arc::clone(&arc));
    drop(cache);
    check_and_purge();
    arc
}

/// Versión opcional: Si recibe Some, interna el string. Si recibe None, devuelve None.
pub fn intern_string_opt(s: Option<&str>) -> Option<std::sync::Arc<str>> {
    s.map(intern_string)
}

/// Limpia por completo la caché de cadenas para recuperar RAM.
pub fn clear_interner() {
    if let Some(mtx) = STRING_CACHE.get() {
        let mut cache = mtx.lock().unwrap();
        cache.clear();
        cache.shrink_to_fit();
    }
    TOTAL_ALLOCATED.store(0, Ordering::Relaxed);
}

/// Devuelve estadísticas de la caché: (entradas, bytes_aproximados)
pub fn interner_stats() -> (usize, u64) {
    let entries = get_cache().lock().map(|c| c.len()).unwrap_or(0);
    let bytes = TOTAL_ALLOCATED.load(Ordering::Relaxed);
    (entries, bytes)
}
