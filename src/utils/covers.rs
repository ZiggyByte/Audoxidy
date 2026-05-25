use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::AvifEncoder;
use image::{ExtendedColorType, ImageEncoder};
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Instancia estática del Thread Pool limitado para tareas de fondo
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Canal limitado para controlar la presión de memoria durante el escaneado
static COVER_GATEWAY: OnceLock<crossbeam::channel::Sender<(Vec<u8>, String)>> = OnceLock::new();

/// Almacena rutas de archivos que fallaron o no existen (evita reintentos)
pub static NEGATIVE_CACHE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Estructura de caché LRU manual para retener Handles y prevenir OOM en Iced
pub struct CoverCache {
    pub map: HashMap<String, iced::widget::image::Handle>,
    pub order: VecDeque<String>,
}

pub static LRU_COVER_CACHE: OnceLock<Mutex<CoverCache>> = OnceLock::new();
const MAX_COVERS_CACHE: usize = 64;

pub fn get_lru_cache() -> &'static Mutex<CoverCache> {
    LRU_COVER_CACHE.get_or_init(|| {
        Mutex::new(CoverCache {
            map: HashMap::with_capacity(MAX_COVERS_CACHE),
            order: VecDeque::with_capacity(MAX_COVERS_CACHE),
        })
    })
}

pub fn get_cover_pool() -> &'static rayon::ThreadPool {
    COVER_POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // Low-resource: máx 1 hilo. Normal: 1/3 de los procesadores (para evitar sobrecalentamiento)
        let pool_size = if crate::utils::is_low_resource() {
            (cores / 3).clamp(1, 1)
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

/// Limpia la caché negativa (archivos que fallaron). Útil tras re-escaneo.
pub fn clear_all_cover_cache() {
    if let Some(neg_cache_mtx) = NEGATIVE_CACHE.get() {
        neg_cache_mtx.lock().clear();
    }
}

/// Genera un ID único para una imagen específica de canción (u8 data)
pub fn generate_pic_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
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

    let src_image = Image::from_vec_u8(
        width,
        height,
        img.into_raw(),
        fast_image_resize::PixelType::U8x4,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let dst_width = 400;
    let dst_height = 400;
    let mut dst_image = Image::new(dst_width, dst_height, src_image.pixel_type());

    let mut resizer = Resizer::new();

    // Activar opcionalmente extensiones de procesador modernas en x86 para super-velocidad
    #[cfg(target_arch = "x86_64")]
    unsafe {
        resizer.set_cpu_extensions(fast_image_resize::CpuExtensions::Avx2);
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

    let handle_opt = cache.map.get(path).cloned();
    if let Some(handle) = handle_opt {
        // Actualizar el orden del LRU (remover de la posición actual y poner al frente)
        if let Some(idx) = cache.order.iter().position(|x| x == path) {
            cache.order.remove(idx);
            cache.order.push_back(path.to_string());
        }
        return Some(handle);
    }

    // 3. Crear nuevo Handle y guardar en caché
    let handle = iced::widget::image::Handle::from_path(path);
    cache.map.insert(path.to_string(), handle.clone());
    cache.order.push_back(path.to_string());

    // 4. Limitar el tamaño a MAX_COVERS_CACHE (150)
    while cache.order.len() > MAX_COVERS_CACHE {
        if let Some(oldest_path) = cache.order.pop_front() {
            cache.map.remove(&oldest_path);
        }
    }

    Some(handle)
}

/// Purga las carátulas más viejas de la caché.
/// Usado por el Garbage Collector cuando la app está inactiva.
pub fn purge_old_covers(count: usize) {
    let cache_mtx = get_lru_cache();
    let mut cache = cache_mtx.lock();
    let to_remove = count.min(cache.order.len());

    for _ in 0..to_remove {
        if let Some(oldest_path) = cache.order.pop_front() {
            cache.map.remove(&oldest_path);
        }
    }
}

/// Carga una imagen desde bytes crudos (fallback del reproductor).
/// Solo se usa cuando no hay carátula en caché de disco (datos embebidos del archivo de audio).
pub fn load_raw_image_for_iced(data: &[u8]) -> Option<iced::widget::image::Handle> {
    if data.is_empty() {
        return None;
    }
    // Delegar a Iced directamente con los bytes crudos
    Some(iced::widget::image::Handle::from_bytes(data.to_vec()))
}

/// Limpia la caché de imágenes crudas del reproductor (llamado al cambiar de canción)
pub fn clear_raw_cache() {
    // Ya no hay caché RAM propia — Iced gestiona internamente
}
