//! Sistema de caché y procesamiento de carátulas de álbumes.
//!
//! Escala imágenes a 400×400 (o 200×200 en modo low-resource),
//! las comprime en formato AVIF al 90% de calidad y las almacena
//! en `cache/covers/` con un LRU cache en RAM para Iced.

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::AvifEncoder;
use image::{ExtendedColorType, ImageEncoder};
use lru::LruCache;
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Instancia estática del Thread Pool limitado para tareas de fondo de carátulas.
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Canal limitado para controlar la presión de memoria durante el escaneado
static COVER_GATEWAY: OnceLock<crossbeam::channel::Sender<(Vec<u8>, String)>> = OnceLock::new();

/// Almacena rutas de archivos que fallaron o no existen (evita reintentos)
pub static NEGATIVE_CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Caché LRU para retener Handles de carátulas y prevenir OOM en Iced.
pub struct CoverCache {
    /// Entradas ordenadas por recencia (la más reciente al frente).
    entries: LruCache<String, iced::widget::image::Handle>,
}

impl CoverCache {
    /// Crea la caché con la capacidad indicada. La capacidad debe ser > 0.
    fn new(cap: usize) -> Self {
        Self {
            entries: LruCache::new(
                NonZeroUsize::new(cap).expect("la capacidad debe ser mayor que cero"),
            ),
        }
    }

    /// Devuelve el Handle y promueve la entrada a la posición más reciente.
    fn get(&mut self, key: &str) -> Option<iced::widget::image::Handle> {
        self.entries.get(key).cloned()
    }

    /// Consulta la entrada sin alterar el orden de recencia.
    #[cfg(test)]
    fn peek(&self, key: &str) -> Option<iced::widget::image::Handle> {
        self.entries.peek(key).cloned()
    }

    /// Inserta o actualiza la entrada, promoviéndola a la posición más reciente.
    fn put(&mut self, key: String, value: iced::widget::image::Handle) {
        self.entries.put(key, value);
    }

    /// Promueve una clave existente sin insertarla si no está presente.
    fn promote(&mut self, key: &str) -> bool {
        self.entries.promote(key)
    }

    /// Ajusta la capacidad; al reducir desaloja primero las menos recientes.
    fn resize(&mut self, cap: usize) {
        if self.entries.cap().get() != cap {
            self.entries
                .resize(NonZeroUsize::new(cap).expect("la capacidad debe ser mayor que cero"));
        }
    }

    /// Número de entradas retenidas.
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Indica si la clave está en la caché, sin alterar el orden de recencia.
    fn contains(&self, key: &str) -> bool {
        self.entries.contains(key)
    }

    /// Orden de más reciente a menos reciente.
    #[cfg(test)]
    fn order_mru_to_lru(&self) -> Vec<String> {
        self.entries.iter().map(|(key, _)| key.clone()).collect()
    }
}

/// Límite dinámico de carátulas en caché según modo:
/// - Normal: 64 (estándar, suficiente para pantallas grandes)
/// - Low-resource: 16 (evicción agresiva para ahorrar RAM)
fn get_max_covers() -> usize {
    if crate::utils::is_low_resource() {
        16
    } else {
        64
    }
}

pub static LRU_COVER_CACHE: OnceLock<Mutex<CoverCache>> = OnceLock::new();

/// Obtiene o inicializa la caché LRU global de carátulas.
pub fn get_lru_cache() -> &'static Mutex<CoverCache> {
    LRU_COVER_CACHE.get_or_init(|| Mutex::new(CoverCache::new(get_max_covers())))
}

/// Obtiene o inicializa el ThreadPool global para procesamiento de carátulas.
///
/// En modo low-resource usa 1 hilo; en modo normal usa 1/3 de los núcleos.
pub fn get_cover_pool() -> &'static rayon::ThreadPool {
    COVER_POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // Low-resource: máx 1 hilo. Normal: 1/3 de los procesadores (para evitar sobrecalentamiento)
        let pool_size = if crate::utils::is_low_resource() {
            1
        } else {
            (cores / 3).max(1)
        };

        ThreadPoolBuilder::new()
            .num_threads(pool_size)
            .stack_size(8 * 1024 * 1024) // 8MB por hilo — estabilidad con AVIF/ravif
            .thread_name(|idx| format!("cover-cache-worker-{}", idx))
            .build()
            .unwrap()
    })
}

/// Encola una tarea de procesamiento de carátula de forma segura.
/// Si hay demasiadas tareas pendientes (16), el hilo del escáner esperará,
/// evitando clonar miles de búferes de imagen en la RAM simultáneamente.
pub fn enqueue_cover_job(data: Vec<u8>, hash: String) {
    let tx = COVER_GATEWAY.get_or_init(|| {
        let (tx, rx) = crossbeam::channel::bounded::<(Vec<u8>, String)>(16);

        let pool = get_cover_pool();

        // Iniciamos hilos persistentes en el pool que consumen del canal
        for _ in 0..pool.current_num_threads() {
            let rx_worker = rx.clone();
            pool.spawn(move || {
                while let Ok((pic, name)) = rx_worker.recv() {
                    let _ = std::panic::catch_unwind(move || {
                        if let Err(e) = process_and_save_cover(&pic, &name) {
                            tracing::warn!(
                                "No se pudo procesar/guardar la carátula '{}': {}",
                                name,
                                e
                            );
                        }
                    });
                }
            });
        }

        tx
    });

    let _ = tx.send((data, hash));
}

/// Vacía el conjunto de rutas que fallaron al cargar.
fn clear_negative_cache(set: &mut HashSet<String>) {
    set.clear();
}

/// Limpia la caché negativa (archivos que fallaron). Útil tras re-escaneo.
pub fn clear_all_cover_cache() {
    if let Some(neg_cache_mtx) = NEGATIVE_CACHE.get() {
        clear_negative_cache(&mut neg_cache_mtx.lock());
    }
}

/// Genera un ID único para una imagen específica de canción (u8 data)
pub fn generate_pic_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Devuelve la extensión de CPU a forzar, o `None` para conservar el default
/// de la librería (que ya selecciona la mejor extensión disponible en runtime).
///
/// Solo se fuerza AVX2 cuando el procesador la soporta; si no, no se toca nada.
fn forced_cpu_extension() -> Option<fast_image_resize::CpuExtensions> {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            return Some(fast_image_resize::CpuExtensions::Avx2);
        }
    }
    None
}

/// Extrae de un buffer raw de imagen, recorta/escala a 400x400 y guarda como .avif 90% de calidad.
pub fn process_and_save_cover(data: &[u8], safe_album_name: &str) -> std::io::Result<PathBuf> {
    let cache_dir = PathBuf::from("cache/covers");
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir)?;
    }

    let dst_path = cache_dir.join(format!("{}.avif", safe_album_name));
    if dst_path.exists() {
        return Ok(dst_path);
    }

    // 1. Cargar imagen en memoria y convertirla genéricamente a RGBA 8 bit
    let img = match image::load_from_memory(data) {
        Ok(i) => i.to_rgba8(),
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to decode image",
            ));
        }
    };

    let width = img.width();
    let height = img.height();

    if width == 0 || height == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Dimensiones inválidas: 0",
        ));
    }

    let target_size = if crate::utils::is_low_resource() {
        200
    } else {
        400
    };

    let src_image = Image::from_vec_u8(
        width,
        height,
        img.into_raw(),
        fast_image_resize::PixelType::U8x4,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let dst_width = target_size;
    let dst_height = target_size;
    let mut dst_image = Image::new(dst_width, dst_height, src_image.pixel_type());

    let mut resizer = Resizer::new();

    if let Some(ext) = forced_cpu_extension() {
        // SAFETY: `forced_cpu_extension` solo devuelve `Avx2` tras comprobar en
        // runtime con `is_x86_feature_detected!("avx2")` que el procesador la
        // soporta, de modo que la extensión está garantizada en este punto.
        unsafe {
            resizer.set_cpu_extensions(ext);
        }
    }

    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Guardar como AVIF con calidad 90 y velocidad rápida (8) para ahorrar RAM/CPU
    let file = std::fs::File::create(&dst_path)?;
    let encoder = AvifEncoder::new_with_speed_quality(file, 8, 90);

    encoder
        .write_image(
            dst_image.buffer(),
            dst_width,
            dst_height,
            ExtendedColorType::Rgba8.into(),
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    Ok(dst_path)
}

/// Carga una carátula desde disco usando Handle::from_path() de Iced.
/// Iced gestiona internamente la decodificación y la memoria.
/// Arquitectura: Disco AVIF → Iced (sin caché RAM intermedia).
pub fn load_cover_handle(path: &str) -> Option<iced::widget::image::Handle> {
    let neg_cache_mtx = NEGATIVE_CACHE.get_or_init(|| Mutex::new(HashSet::new()));

    // 1. Verificar si el archivo ya falló anteriormente
    if neg_cache_mtx.lock().contains(path) {
        return None;
    }

    // 2. Revisar la Caché LRU en memoria
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();

    // Recalcular límite dinámico en cada carga (el modo pudo cambiar); al reducir,
    // `resize` desaloja primero las entradas menos recientes.
    cache.resize(get_max_covers());

    if let Some(handle) = cache.get(path) {
        return Some(handle);
    }

    // 3. Crear nuevo Handle y guardar en caché; `put` desaloja si excede la capacidad.
    let handle = iced::widget::image::Handle::from_path(path);
    cache.put(path.to_string(), handle.clone());

    Some(handle)
}

/// Desaloja las entradas menos recientes. En modo low-resource
/// duplica el número de entradas a purgar.
fn purge_oldest_covers(cache: &mut CoverCache, count: usize, low_resource: bool) {
    let effective_count = if low_resource { count * 2 } else { count };
    let to_remove = effective_count.min(cache.entries.len());

    for _ in 0..to_remove {
        cache.entries.pop_lru();
    }
}

/// Purga las carátulas más viejas de la caché.
/// Usado por el Garbage Collector cuando la app está inactiva.
/// En low-resource, purga el doble de elementos por ciclo.
pub fn purge_old_covers(count: usize) {
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    purge_oldest_covers(&mut cache, count, crate::utils::is_low_resource());
}

/// Carga una imagen desde bytes crudos (fallback del reproductor).
/// Precarga carátulas para una lista de rutas de álbumes visibles.
/// Útil para evitar frames en blanco al hacer scroll por la biblioteca:
/// las carátulas se cargan en la LRU cache antes de que el usuario las vea.
pub fn preload_visible_covers(paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    for path in paths {
        if cache.contains(path) {
            // Ya en caché: promover a MRU sin reinsertar.
            cache.promote(path);
        } else {
            // No está: cargar desde disco y guardar en LRU.
            let handle = iced::widget::image::Handle::from_path(path);
            cache.put(path.clone(), handle);
        }
    }
}

/// Solo se usa cuando no hay carátula en caché de disco (datos embebidos del archivo de audio).
/// Carga una imagen desde bytes crudos para usar en Iced (fallback para datos embebidos).
pub fn load_raw_image_for_iced(data: &[u8]) -> Option<iced::widget::image::Handle> {
    if data.is_empty() {
        return None;
    }
    Some(iced::widget::image::Handle::from_bytes(data.to_vec()))
}

/// Vacía por completo la caché LRU de carátulas.
fn clear_cover_cache(cache: &mut CoverCache) {
    cache.entries.clear();
}

/// Limpia la caché de imágenes crudas del reproductor (llamado al cambiar de canción)
/// Limpia la caché LRU completa de carátulas (llamado al cambiar de canción).
pub fn clear_raw_cache() {
    // Vaciar LRU completo al limpiar
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    clear_cover_cache(&mut cache);
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::image::Handle;
    use std::collections::{HashMap, VecDeque};

    /// Handle determinista y comparable: `from_path` deriva el `Id` de la ruta,
    /// a diferencia de `from_bytes`, que genera identidades únicas.
    fn test_handle(i: usize) -> Handle {
        Handle::from_path(format!("cache/covers/{i}.avif"))
    }

    /// Operaciones mínimas que el arnés de paridad necesita de cada implementación.
    trait ParityCache {
        fn get(&mut self, key: &str) -> Option<Handle>;
        fn peek(&self, key: &str) -> Option<Handle>;
        fn insert(&mut self, key: String, value: Handle);
        fn clear(&mut self);
        fn purge_oldest(&mut self, count: usize, low_resource: bool);
        fn resize(&mut self, cap: usize);
        fn len(&self) -> usize;
        fn contains(&self, key: &str) -> bool;
        /// Orden de más reciente a menos reciente.
        fn order_mru_to_lru(&self) -> Vec<String>;
    }

    /// Adaptador de paridad sobre la caché de producción (respaldada por `lru`).
    struct ProductionCache {
        cache: CoverCache,
    }

    impl ProductionCache {
        fn new(cap: usize) -> Self {
            Self {
                cache: CoverCache::new(cap),
            }
        }
    }

    impl ParityCache for ProductionCache {
        fn get(&mut self, key: &str) -> Option<Handle> {
            self.cache.get(key)
        }

        fn peek(&self, key: &str) -> Option<Handle> {
            self.cache.peek(key)
        }

        fn insert(&mut self, key: String, value: Handle) {
            self.cache.put(key, value);
        }

        fn clear(&mut self) {
            clear_cover_cache(&mut self.cache);
        }

        fn purge_oldest(&mut self, count: usize, low_resource: bool) {
            purge_oldest_covers(&mut self.cache, count, low_resource);
        }

        fn resize(&mut self, cap: usize) {
            self.cache.resize(cap);
        }

        fn len(&self) -> usize {
            self.cache.len()
        }

        fn contains(&self, key: &str) -> bool {
            self.cache.contains(key)
        }

        fn order_mru_to_lru(&self) -> Vec<String> {
            self.cache.order_mru_to_lru()
        }
    }

    /// Congela la semántica de la caché manual previa a la adopción de `lru`
    /// (mapa de claves a `Handle` + cola de orden de acceso). Es una referencia
    /// independiente de la implementación de producción: una divergencia real
    /// en capacidad, recencia, desalojo, purga o resize hace fallar el test.
    struct ReferenceCache {
        map: HashMap<String, Handle>,
        order: VecDeque<String>,
        cap: usize,
    }

    impl ReferenceCache {
        fn new(cap: usize) -> Self {
            Self {
                map: HashMap::with_capacity(cap),
                order: VecDeque::with_capacity(cap),
                cap,
            }
        }
    }

    impl ParityCache for ReferenceCache {
        fn get(&mut self, key: &str) -> Option<Handle> {
            let handle = self.map.get(key).cloned()?;
            // El acierto promueve: la clave se mueve al final de la cola.
            if let Some(idx) = self.order.iter().position(|x| x == key) {
                self.order.remove(idx);
                self.order.push_back(key.to_string());
            }
            Some(handle)
        }

        fn peek(&self, key: &str) -> Option<Handle> {
            self.map.get(key).cloned()
        }

        fn insert(&mut self, key: String, value: Handle) {
            if self.map.insert(key.clone(), value).is_some() {
                // Reinsertar una clave existente también la promueve.
                if let Some(idx) = self.order.iter().position(|x| x == &key) {
                    self.order.remove(idx);
                }
            }
            self.order.push_back(key);
            while self.order.len() > self.cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn clear(&mut self) {
            self.map.clear();
            self.order.clear();
        }

        fn purge_oldest(&mut self, count: usize, low_resource: bool) {
            let effective_count = if low_resource { count * 2 } else { count };
            let to_remove = effective_count.min(self.order.len());
            for _ in 0..to_remove {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn resize(&mut self, cap: usize) {
            self.cap = cap;
            while self.order.len() > self.cap {
                if let Some(oldest) = self.order.pop_front() {
                    self.map.remove(&oldest);
                }
            }
        }

        fn len(&self) -> usize {
            self.order.len()
        }

        fn contains(&self, key: &str) -> bool {
            self.map.contains_key(key)
        }

        fn order_mru_to_lru(&self) -> Vec<String> {
            self.order.iter().rev().cloned().collect()
        }
    }

    /// Ejecuta la misma secuencia contra la caché de producción y la referencia
    /// manual independiente, y exige que el rastro observable y el orden final
    /// coincidan. La referencia no usa `lru`, así que una divergencia de la
    /// implementación real contra la semántica documentada hace fallar el test.
    fn assert_parity(cap: usize, scenario: impl Fn(&mut dyn ParityCache) -> Vec<String>) {
        let mut production = ProductionCache::new(cap);
        let mut reference = ReferenceCache::new(cap);

        let production_trace = scenario(&mut production);
        let reference_trace = scenario(&mut reference);

        assert_eq!(
            production_trace, reference_trace,
            "rastro divergente en cap={cap}"
        );
        assert_eq!(
            production.order_mru_to_lru(),
            reference.order_mru_to_lru(),
            "orden divergente en cap={cap}"
        );
    }

    /// Contrato explícito de recencia, acierto y desalojo, sin comparar contra
    /// ninguna otra implementación: fija el orden MRU→LRU esperado.
    #[test]
    fn covercache_golden_lru_order() {
        let mut cache = CoverCache::new(3);
        cache.put("a".into(), test_handle(0));
        cache.put("b".into(), test_handle(1));
        cache.put("c".into(), test_handle(2));
        assert_eq!(cache.order_mru_to_lru().join(","), "c,b,a");

        let _ = cache.get("a");
        assert_eq!(cache.order_mru_to_lru().join(","), "a,c,b");

        // `peek` consulta sin alterar el orden de recencia.
        assert!(cache.peek("b").is_some());
        assert_eq!(cache.order_mru_to_lru().join(","), "a,c,b");

        // Insertar desaloja la menos reciente ("b"), no la recién leída.
        cache.put("d".into(), test_handle(3));
        assert_eq!(cache.order_mru_to_lru().join(","), "d,a,c");
        assert!(!cache.contains("b"));
    }

    /// Inserta `cap` claves numeradas y devuelve sus nombres en orden de inserción.
    fn fill(cache: &mut dyn ParityCache, cap: usize) -> Vec<String> {
        let keys: Vec<String> = (0..cap).map(|i| format!("k{i}")).collect();
        for (i, key) in keys.iter().enumerate() {
            cache.insert(key.clone(), test_handle(i));
        }
        keys
    }

    #[test]
    fn covercache_capacity_bound_parity() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                for i in 0..cap + 5 {
                    cache.insert(format!("k{i}"), test_handle(i));
                }
                vec![format!("len={}", cache.len())]
            });
        }
    }

    #[test]
    fn covercache_recency_eviction_order_parity() {
        for cap in [3usize, 4, 16, 64] {
            assert_parity(cap, |cache| {
                let keys = fill(cache, cap);
                let first = keys[0].clone();
                let second = keys[1].clone();
                // Leer la más antigua la promueve; insertar una nueva desaloja la
                // siguiente más antigua, no la recién leída.
                let _ = cache.get(&first);
                cache.insert("nueva".into(), test_handle(999));
                vec![
                    format!("len={}", cache.len()),
                    format!("first_present={}", cache.contains(&first)),
                    format!("second_present={}", cache.contains(&second)),
                    format!("order={:?}", cache.order_mru_to_lru()),
                ]
            });
        }
    }

    #[test]
    fn covercache_get_promotes_but_peek_does_not() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                let keys = fill(cache, cap.min(3));
                let before = cache.order_mru_to_lru();

                let _ = cache.peek(&keys[0]);
                let after_peek = cache.order_mru_to_lru();

                let _ = cache.get(&keys[0]);
                let after_get = cache.order_mru_to_lru();

                vec![
                    format!("peek_preserves_order={}", before == after_peek),
                    format!(
                        "get_promotes_to_mru={}",
                        after_get.first() == Some(&keys[0])
                    ),
                ]
            });
        }
    }

    /// Estrés de concurrencia bajo un único `Mutex`: ningún hilo puede dejar la
    /// caché por encima de la capacidad ni provocar un pánico.
    fn concurrency_stress<C: ParityCache + Send>(mutex: &Mutex<C>, cap: usize) {
        std::thread::scope(|scope| {
            for thread in 0..8usize {
                scope.spawn(move || {
                    for i in 0..200usize {
                        let key = format!("/music/album/{thread}/{i}");
                        let mut guard = mutex.lock();
                        guard.insert(key.clone(), test_handle(i));
                        let _ = guard.get(&key);
                        let _ = guard.get("/music/album/ausente");
                        assert!(guard.len() <= cap, "la caché excedió su capacidad");
                    }
                });
            }
        });
        assert!(mutex.lock().len() <= cap);
    }

    #[test]
    fn covercache_concurrency_parity() {
        let cap = 8;
        concurrency_stress(&Mutex::new(ProductionCache::new(cap)), cap);
        concurrency_stress(&Mutex::new(ReferenceCache::new(cap)), cap);
    }

    #[test]
    fn covercache_clear_parity() {
        for cap in [4usize, 16, 64] {
            assert_parity(cap, |cache| {
                fill(cache, cap);
                cache.clear();
                vec![
                    format!("len={}", cache.len()),
                    format!("order={:?}", cache.order_mru_to_lru()),
                ]
            });
        }
    }

    #[test]
    fn covercache_negative_cache_clear_is_isolated() {
        let mut negative: HashSet<String> = HashSet::new();
        for i in 0..3 {
            negative.insert(format!("missing-{i}"));
        }

        // Ambas implementaciones parten con entradas que no deben verse afectadas.
        let mut production = ProductionCache::new(4);
        production.insert("keep".into(), test_handle(1));
        let mut reference = ReferenceCache::new(4);
        reference.insert("keep".into(), test_handle(1));

        clear_negative_cache(&mut negative);

        assert!(
            negative.is_empty(),
            "el conjunto negativo debe quedar vacío"
        );
        // Limpiar el conjunto negativo es independiente de la caché LRU en ambos casos.
        assert_eq!(production.len(), 1);
        assert_eq!(reference.len(), 1);
    }

    #[test]
    fn covercache_purge_parity() {
        for cap in [4usize, 16] {
            for (count, low_resource) in [(1usize, false), (2usize, true)] {
                assert_parity(cap, |cache| {
                    fill(cache, cap);
                    let newest = format!("k{}", cap - 1);
                    cache.purge_oldest(count, low_resource);

                    let removed = if low_resource { count * 2 } else { count }.min(cap);
                    // La purga quita las menos recientes; la más nueva sobrevive
                    // mientras no se haya purgado la caché entera.
                    assert_eq!(cache.len(), cap - removed);
                    if removed < cap {
                        assert!(cache.contains(&newest));
                        assert!(!cache.contains("k0"));
                    } else {
                        assert!(cache.order_mru_to_lru().is_empty());
                    }

                    vec![
                        format!("len={}", cache.len()),
                        format!("order={:?}", cache.order_mru_to_lru()),
                    ]
                });
            }
        }
    }

    #[test]
    fn covercache_resize_parity() {
        for start in [16usize, 64] {
            for target in [4usize, 8] {
                assert_parity(start, |cache| {
                    fill(cache, start);
                    let newest = format!("k{}", start - 1);

                    // Reducir la capacidad desaloja primero las menos recientes.
                    cache.resize(target);
                    assert_eq!(
                        cache.len(),
                        target,
                        "reducir debe respetar la nueva capacidad"
                    );
                    assert!(cache.contains(&newest), "la más reciente debe sobrevivir");
                    assert!(!cache.contains("k0"), "la menos reciente debe desalojarse");
                    let shrink_order = cache.order_mru_to_lru();

                    // Ampliar la capacidad no desaloja ninguna entrada.
                    cache.resize(start + 16);
                    assert_eq!(cache.len(), target, "ampliar no debe desalojar");

                    vec![
                        format!("shrink_len={target}"),
                        format!("order={shrink_order:?}"),
                        format!("grow_len={}", cache.len()),
                    ]
                });
            }
        }
    }

    #[test]
    fn avx2_selection_matches_detection() {
        let forced = forced_cpu_extension();
        // En x86_64: forzamos Avx2 solo si el host lo soporta.
        #[cfg(target_arch = "x86_64")]
        assert_eq!(forced.is_some(), std::is_x86_feature_detected!("avx2"));
        // Fuera de x86_64 nunca forzamos nada.
        #[cfg(not(target_arch = "x86_64"))]
        assert!(forced.is_none());
    }

    #[test]
    fn process_and_save_cover_smoke() {
        // Imagen RGBA en memoria → PNG en bytes, para recorrer el camino real
        // de decodificación, redimensionado y codificación AVIF.
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("no se pudo codificar el PNG de prueba");

        let path = process_and_save_cover(&bytes, "audoxidy_smoke_test_cover")
            .expect("process_and_save_cover falló en el smoke test");

        assert!(path.exists(), "la carátula AVIF no se creó en disco");
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("avif"));

        // Limpia solo el archivo generado; el directorio cache/covers/ es un
        // artefacto de runtime ignorado por git y se deja en su sitio.
        let _ = std::fs::remove_file(&path);
    }
}
