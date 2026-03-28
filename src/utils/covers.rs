use std::path::PathBuf;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use parking_lot::Mutex;
use sha2::{Sha256, Digest};
use std::time::{Instant, Duration};
use image::{load_from_memory, ExtendedColorType, ImageEncoder};
use image::codecs::avif::AvifEncoder;
use fast_image_resize::images::Image;
use fast_image_resize::{Resizer, ResizeOptions, FilterType, ResizeAlg};
use rayon::ThreadPoolBuilder;

/// Estructura para gestionar el TTL de una carátula en RAM
pub struct CachedHandle {
    pub handle: iced::widget::image::Handle,
    pub last_access: Instant,
}

/// Instancia estática del Thread Pool limitado para tareas de fondo
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Canal limitado para controlar la presión de memoria durante el escaneado
static COVER_GATEWAY: OnceLock<crossbeam::channel::Sender<(Vec<u8>, String)>> = OnceLock::new();

/// Caché estática gráfica en memoria para listados de Iced (Con TTL de 1 minuto)
pub static IMAGE_HANDLE_CACHE: OnceLock<Mutex<HashMap<String, CachedHandle>>> = OnceLock::new();
/// Caché para imágenes temporales desde bytes (como las del reproductor)
pub static RAW_HANDLE_CACHE: OnceLock<Mutex<HashMap<u64, iced::widget::image::Handle>>> = OnceLock::new();
/// Almacena rutas de archivos que fallaron al decodificar
pub static NEGATIVE_CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

const MEMORY_TTL: Duration = Duration::from_secs(30); // Reducido a 30s para mayor agresividad
const MAX_CACHE_SIZE: usize = 50; // Límite de carátulas simultáneas en RAM (Grid usa ~50)

pub fn get_cover_pool() -> &'static rayon::ThreadPool {
    COVER_POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let target = (cores * 2) / 3;
        let pool_size = if target == 0 { 1 } else { target };

        ThreadPoolBuilder::new()
            .num_threads(pool_size)
            .stack_size(8 * 1024 * 1024) // 8MB por hilo para máxima estabilidad con AVIF/ravif
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
        // Los Receiver de crossbeam se pueden clonar y compartir entre hilos directamente
        for _ in 0..pool.current_num_threads() {
            let rx_worker = rx.clone();
            pool.spawn(move || {
                while let Ok((pic, name)) = rx_worker.recv() {
                    let _ = std::panic::catch_unwind(move || {
                        let _ = process_and_save_cover(&pic, &name);
                    });
                }
            });
        }
        
        tx
    });

    // Esta llamada BLOQUEA al escáner si el canal (16 slots) está lleno
    let _ = tx.send((data, hash));
}

/// Libera manualmente toda la memoria RAM ocupada por los Handles que han superado el TTL.
pub fn clear_expired_covers() {
    if let Some(cache_mtx) = IMAGE_HANDLE_CACHE.get() {
        let mut cache = cache_mtx.lock();
        let now = Instant::now();
        
        // 1. Eliminar por tiempo
        cache.retain(|_, v| now.duration_since(v.last_access) < MEMORY_TTL);
        
        // 2. Si aún supera el límite, eliminar los más antiguos (LRU simplificado)
        if cache.len() > MAX_CACHE_SIZE {
            let mut items: Vec<_> = cache.iter().map(|(k, v)| (k.clone(), v.last_access)).collect();
            items.sort_by_key(|&(_, last)| last);
            let to_remove = items.len() - MAX_CACHE_SIZE;
            for i in 0..to_remove {
                cache.remove(&items[i].0);
            }
        }
    }
}

/// Limpia la caché de imágenes crudas del reproductor (llamado al cambiar de canción)
pub fn clear_raw_cache() {
    if let Some(cache_mtx) = RAW_HANDLE_CACHE.get() {
        cache_mtx.lock().clear();
    }
}

pub fn clear_all_cover_cache() {
    if let Some(cache_mtx) = IMAGE_HANDLE_CACHE.get() {
        cache_mtx.lock().clear();
    }
    if let Some(neg_cache_mtx) = NEGATIVE_CACHE.get() {
        neg_cache_mtx.lock().clear();
    }
}

/// Genera un ID único para un álbum basado exclusivamente en el Hash SHA256 de Artista + Álbum
pub fn generate_album_id(artist: &str, album: &str) -> String {
    let mut hasher = Sha256::new();
    // Normalizamos a minúsculas para evitar duplicados por capitalización
    hasher.update(format!("{}|{}", artist.to_lowercase(), album.to_lowercase()).as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16]) // 32 caracteres hexadecimales para colisiones nulas
}

/// Genera un ID único para una imagen específica de canción (u8 data)
pub fn generate_song_art_id(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{}_song", hex::encode(&result[..8]))
}

/// Extrae de un buffer raw de imagen, recorta/escala a 512x512 y guarda como .avif 80% de calidad.
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
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to decode image")),
    };

    let width = img.width();
    let height = img.height();
    
    if width == 0 || height == 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Dimensiones inválidas: 0"));
    }
    
    let src_image = Image::from_vec_u8(width, height, img.into_raw(), fast_image_resize::PixelType::U8x4)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let dst_width = 512;
    let dst_height = 512;
    let mut dst_image = Image::new(dst_width, dst_height, src_image.pixel_type());

    let mut resizer = Resizer::new();
    
    // Activar opcionalmente extensiones de procesador modernas en x86 para super-velocidad
    #[cfg(target_arch = "x86_64")]
    unsafe { resizer.set_cpu_extensions(fast_image_resize::CpuExtensions::Avx2); }

    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));

    resizer.resize(&src_image, &mut dst_image, &options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Guardar como AVIF con calidad 80 y velocidad rápida (8) para ahorrar RAM/CPU
    let file = std::fs::File::create(&dst_path)?;
    let encoder = AvifEncoder::new_with_speed_quality(file, 8, 80);
    
    encoder.write_image(dst_image.buffer(), dst_width, dst_height, ExtendedColorType::Rgba8.into())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    Ok(dst_path)
}

/// Lee de forma síncrona desde el disco cualquier imagen (avif, png, jpeg) decodificándola
/// con el crate interno de `image` y lo transforma en pixéles crudos (RGBA8) compatibles
/// con el buffer que `iced` necesita. Ahora utiliza memoria local Hash para evitar recargas en Iced Layouts!
/// Esta es la solución puente definitiva para mostrar AVIF nativo.
/// Lee de forma síncrona desde el disco cualquier imagen (avif, png, jpeg) decodificándola
/// Implementa una política de RAM estricta: Actualiza last_access para el TTL.
pub fn load_image_for_iced(path: &str) -> Option<iced::widget::image::Handle> {
    let cache_mtx = IMAGE_HANDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let neg_cache_mtx = NEGATIVE_CACHE.get_or_init(|| Mutex::new(HashSet::new()));

    // 1. Verificar si el archivo ya falló anteriormente
    if neg_cache_mtx.lock().contains(path) {
        return None;
    }

    // 2. Verificar caché positiva y actualizar acceso
    let mut cache = cache_mtx.lock();
    if let Some(cached) = cache.get_mut(path) {
        cached.last_access = Instant::now();
        return Some(cached.handle.clone());
    }

    // Carga desde disco
    if let Ok(bytes) = std::fs::read(path) {
        match load_from_memory(&bytes) {
            Ok(img) => {
                let rgba = img.into_rgba8();
                let (width, height) = rgba.dimensions();
                let handle = iced::widget::image::Handle::from_rgba(
                    width,
                    height,
                    rgba.into_raw(),
                );
                cache.insert(path.to_string(), CachedHandle {
                    handle: handle.clone(),
                    last_access: Instant::now(),
                });
                return Some(handle);
            }
            Err(_) => {
                neg_cache_mtx.lock().insert(path.to_string());
            }
        }
    }
    None
}

/// Función de conveniencia para cargar imágenes sin caché (carga directa disco -> RAM temporal)
pub fn load_direct_from_disk(path: &str) -> Option<iced::widget::image::Handle> {
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(img) = image::load_from_memory(&bytes) {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            return Some(iced::widget::image::Handle::from_rgba(width, height, rgba.into_raw()));
        }
    }
    None
}

/// Carga una imagen desde bytes crudos (fallback) utilizando una caché basada en hash
/// para evitar que la interfaz se bloquee al clonar búferes pesados en el ciclo de dibujo.
pub fn load_raw_image_for_iced(data: &[u8]) -> Option<iced::widget::image::Handle> {
    if data.is_empty() { return None; }
    
    let cache_mtx = RAW_HANDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    
    // Generar un hash rápido del contenido para usarlo como ID
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    let id = hasher.finish();

    let mut cache = cache_mtx.lock();
    if let Some(handle) = cache.get(&id) {
        return Some(handle.clone());
    }

    if cache.len() >= 50 { // Caché cruda más pequeña para ahorrar RAM
        cache.clear();
    }

    match load_from_memory(data) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            let handle = iced::widget::image::Handle::from_rgba(
                width,
                height,
                rgba.into_raw(),
            );
            cache.insert(id, handle.clone());
            Some(handle)
        }
        Err(_) => None
    }
}
