use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;

static STRING_CACHE: OnceLock<Mutex<HashMap<String, Arc<str>>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<String, Arc<str>>> {
    STRING_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Devuelve un puntero Arc<str> a la cadena proporcionada.
/// Si la cadena ya existe en el caché, se devuelve un clon del puntero existente, ahorrando memoria.
/// Si no existe, se inserta en el caché y se devuelve.
pub fn intern_string(s: &str) -> std::sync::Arc<str> {
    let mut cache = get_cache().lock().unwrap();
    if let Some(arc) = cache.get(s) {
        return std::sync::Arc::clone(arc);
    }
    let arc: std::sync::Arc<str> = std::sync::Arc::from(s);
    cache.insert(s.to_string(), std::sync::Arc::clone(&arc));
    arc
}

/// Versión opcional: Si recibe Some, interna el string. Si recibe None, devuelve None.
pub fn intern_string_opt(s: Option<&str>) -> Option<std::sync::Arc<str>> {
    s.map(intern_string)
}
