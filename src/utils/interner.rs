use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashSet;

static STRING_CACHE: OnceLock<Mutex<HashSet<Arc<str>>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashSet<Arc<str>>> {
    STRING_CACHE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Devuelve un puntero Arc<str> a la cadena proporcionada.
/// Optimizado para guardar la cadena una sola vez en memoria (HashSet de Arcs).
pub fn intern_string(s: &str) -> std::sync::Arc<str> {
    let mut cache = get_cache().lock().unwrap();
    if let Some(existing) = cache.get(s) {
        return Arc::clone(existing);
    }
    let arc: Arc<str> = Arc::from(s);
    cache.insert(Arc::clone(&arc));
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
}
