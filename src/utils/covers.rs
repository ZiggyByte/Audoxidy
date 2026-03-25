use std::path::PathBuf;
use image::{ExtendedColorType, ImageEncoder};
use image::codecs::avif::AvifEncoder;
use fast_image_resize::{Resizer, ResizeOptions, ResizeAlg, FilterType};
use fast_image_resize::images::Image;
use rayon::ThreadPoolBuilder;

use std::sync::OnceLock;

/// Instancia estática del Thread Pool limitado al 66% de procesamiento para tareas de fondo
pub static COVER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

pub fn get_cover_pool() -> &'static rayon::ThreadPool {
    COVER_POOL.get_or_init(|| {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let target = (cores * 2) / 3;
        let pool_size = if target == 0 { 1 } else { target };

        ThreadPoolBuilder::new()
            .num_threads(pool_size)
            .thread_name(|idx| format!("cover-cache-worker-{}", idx))
            .build()
            .unwrap()
    })
}

/// Extrae de un buffer raw de imagen, recorta/escala a 512x512 y guarda como .avif 65% de calidad.
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

    // 3. Codificar a formato .avif usando 65% de calidad y velocidad intermedia-rápida (6)
    let file = std::fs::File::create(&dst_path)?;
    let encoder = AvifEncoder::new_with_speed_quality(file, 6, 65);
    
    encoder.write_image(dst_image.buffer(), dst_width, dst_height, ExtendedColorType::Rgba8)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    Ok(dst_path)
}
