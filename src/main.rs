mod audio;
mod db;
mod gui;
mod integrations;
mod utils;

#[cfg(test)]
#[path = "audio/dsp_tests.rs"]
mod dsp_tests;

use crate::audio::AudioManager;
use crate::gui::app::AudoxidyApp;
use std::path::PathBuf;

/// Inicializa el sistema de logging estructurado con tracing.
/// - Niveles configurables por módulo
/// - Rotación de archivos
/// - Salida combinada: archivo + stderr
fn init_logging(config: &crate::utils::config::LoggingConfig) {
    use tracing_subscriber::filter::EnvFilter;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let log_dir = PathBuf::from(&config.directory);
    std::fs::create_dir_all(&log_dir).ok();

    // Filtro por módulos: niveles configurables
    let filter = EnvFilter::new(
        config.level.as_str(),
    )
        // Silenciar crates ruidosos en desarrollo
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("reqwest=warn".parse().unwrap())
        .add_directive("cpal=warn".parse().unwrap())
        .add_directive("symphonia=warn".parse().unwrap())
        .add_directive("iced=info".parse().unwrap())
        .add_directive("rusqlite=warn".parse().unwrap())
        .add_directive("mio=warn".parse().unwrap())
        .add_directive("want=warn".parse().unwrap());

    // Log a archivo con rotación por tamaño
    let log_path = log_dir.join("audoxidy.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let file_layer = log_file.map(|file| {
        let max_bytes = config.max_file_size_mb * 1024 * 1024;
        // Rotación simple: si el archivo excede el tamaño, lo renombra con timestamp
        if let Ok(meta) = std::fs::metadata(&log_path) {
            if meta.len() > max_bytes {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let rotated = log_dir.join(format!("audoxidy.{}.log", ts));
                std::fs::rename(&log_path, &rotated).ok();
                cleanup_old_logs(&log_dir, config.max_history_files);
            }
        }
        fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
    });

    // Capa stderr para desarrollo
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(true);

    // Registrar métricas de rendimiento como subscriber adicional
    let metrics_layer = MetricsLayer::new();

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(metrics_layer);

    if let Some(file_layer) = file_layer {
        subscriber.with(file_layer).init();
    } else {
        subscriber.init();
    }
}

/// Limpia logs históricos viejos manteniendo solo los N más recientes.
fn cleanup_old_logs(log_dir: &PathBuf, max_history: u32) {
    let mut logs: Vec<_> = std::fs::read_dir(log_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("audoxidy.") && n.ends_with(".log") && n != "audoxidy.log")
                .unwrap_or(false)
        })
        .collect();
    logs.sort_by_key(|e| e.path());
    while logs.len() > max_history as usize {
        if let Some(oldest) = logs.first() {
            std::fs::remove_file(oldest.path()).ok();
        }
        logs.remove(0);
    }
}

// ── Colector de métricas de rendimiento ──────────────────────

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

/// Métricas globales de rendimiento accesibles desde cualquier módulo.
pub struct PerformanceMetrics {
    /// Picos de latencia de audio (microsegundos)
    pub audio_latency_peak_us: AtomicU64,
    /// Frames de audio procesados
    pub audio_frames_processed: AtomicU64,
    /// Cuenta de underruns
    pub underrun_count: AtomicU64,
    /// Uso de RAM (bytes) — actualizado periódicamente
    pub ram_usage_bytes: AtomicU64,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            audio_latency_peak_us: AtomicU64::new(0),
            audio_frames_processed: AtomicU64::new(0),
            underrun_count: AtomicU64::new(0),
            ram_usage_bytes: AtomicU64::new(0),
        }
    }
}

pub static METRICS: OnceLock<PerformanceMetrics> = OnceLock::new();

pub fn get_metrics() -> &'static PerformanceMetrics {
    METRICS.get_or_init(PerformanceMetrics::new)
}

/// Layer de tracing que captura métricas de rendimiento.
struct MetricsLayer;

impl MetricsLayer {
    fn new() -> Self {
        Self
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for MetricsLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Actualizar métricas cuando se emiten eventos específicos
        let meta = event.metadata();
        if meta.target().contains("audio_latency") {
            // El evento de latencia incluye el valor en microsegundos
            let mut visitor = MetricsVisitor::default();
            event.record(&mut visitor);
            if let Some(latency_us) = visitor.latency_us {
                let metrics = get_metrics();
                let prev = metrics.audio_latency_peak_us.load(Ordering::Relaxed);
                if latency_us > prev {
                    metrics.audio_latency_peak_us.store(latency_us, Ordering::Relaxed);
                }
            }
        }
        if meta.target().contains("underrun") {
            get_metrics().underrun_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[derive(Default)]
struct MetricsVisitor {
    latency_us: Option<u64>,
}

impl tracing::field::Visit for MetricsVisitor {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "latency_us" {
            self.latency_us = Some(value);
        }
    }
    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
}

// ── Entry point ──────────────────────────────────────────────

fn main() -> iced::Result {
    // 1. Cargar configuración (crea archivo por defecto si no existe)
    let app_config = crate::utils::config::load_config();

    // 2. Inicializar logging con configuración
    init_logging(&app_config.logging);

    tracing::info!(
        "Audoxidy {} iniciando con perfil {:?}",
        env!("CARGO_PKG_VERSION"),
        app_config.profile
    );

    // 3. Inicializar gestor de memoria
    crate::utils::memory_manager::MemoryManager::init();

    // 4. Inicializar motor de audio
    let audio_manager = std::sync::Arc::new(
        AudioManager::new().expect("No se pudo inicializar el motor de audio"),
    );

    let window_size = app_config.ui.window_size();

    tracing::info!(
        "Audoxidy listo. Ventana: {}x{}, Audio perfil: {:?}",
        window_size.0, window_size.1, app_config.profile
    );

    // 5. Lanzar aplicación Iced
    iced::application(
        move || AudoxidyApp::new(audio_manager.clone()),
        AudoxidyApp::update,
        AudoxidyApp::view,
    )
    .font(include_bytes!("../assets/fonts/Inter.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Stage-Wanders.ttf").as_slice())
    .title(|_state: &AudoxidyApp| String::from("Audoxidy"))
    .subscription(|state: &AudoxidyApp| state.subscription())
    .theme(|state: &AudoxidyApp| state.theme())
    .window(iced::window::Settings {
        size: iced::Size::new(window_size.0, window_size.1),
        min_size: Some(iced::Size::new(900.0, 500.0)),
        decorations: false,
        transparent: true,
        ..Default::default()
    })
    .run()
}
