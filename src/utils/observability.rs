//! Bootstrap de observabilidad: configuración del subscriber de `tracing`
//! (sinks de archivo y stderr con rotación) y colector de métricas de
//! rendimiento alimentado por eventos estructurados.
//!
//! Los eventos de métricas se emiten con `target: "audoxidy::metrics"` y un
//! campo `metric = "<nombre>"` más un campo numérico `u64`; `MetricsLayer`
//! conmuta sobre ese campo para actualizar los contadores atómicos.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use tracing_subscriber::filter::{EnvFilter, filter_fn};
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::{Layer, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;

/// Identificador de `target` que enruta los eventos hacia `MetricsLayer`.
const METRICS_TARGET: &str = "audoxidy::metrics";

/// Inicializa el sistema de logging estructurado con tracing.
/// - Niveles configurables por módulo
/// - Rotación de archivos
/// - Salida combinada: archivo + stderr
///
/// Es idempotente: usa `try_init` para no entrar en pánico si ya existe un
/// subscriber global (por ejemplo, en pruebas).
pub fn init_logging(config: &crate::utils::config::LoggingConfig) {
    let log_dir = PathBuf::from(&config.directory);
    std::fs::create_dir_all(&log_dir).ok();

    // Filtro por módulos: niveles configurables. Los eventos de métricas
    // siempre pasan el filtro global mediante una directiva propia.
    let filter = EnvFilter::new(config.level.as_str())
        // Silenciar crates ruidosos en desarrollo
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("reqwest=warn".parse().unwrap())
        .add_directive("cpal=warn".parse().unwrap())
        .add_directive("symphonia=warn".parse().unwrap())
        .add_directive("iced=info".parse().unwrap())
        .add_directive("rusqlite=warn".parse().unwrap())
        .add_directive("mio=warn".parse().unwrap())
        .add_directive("want=warn".parse().unwrap())
        .add_directive("audoxidy::metrics=trace".parse().unwrap());

    // Log a archivo con rotación por tamaño. La rotación se resuelve ANTES de
    // abrir el escritor: si se renombra el archivo con el descriptor ya abierto,
    // las escrituras siguen yendo al archivo rotado y `audoxidy.log` no se
    // recrea hasta reiniciar el proceso.
    let log_path = log_dir.join("audoxidy.log");
    let max_bytes = config.max_file_size_mb * 1024 * 1024;
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
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let file_layer = log_file.map(|file| {
        fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::CLOSE)
            .with_filter(filter_fn(|meta| meta.target() != METRICS_TARGET))
    });

    // Capa stderr para desarrollo
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(true)
        .with_filter(filter_fn(|meta| meta.target() != METRICS_TARGET));

    // Registra las métricas de rendimiento como subscriber adicional: solo
    // recibe los eventos cuyo target es el de métricas.
    let metrics_layer = MetricsLayer::new(get_metrics())
        .with_filter(filter_fn(|meta| meta.target() == METRICS_TARGET));

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(metrics_layer);

    if let Some(file_layer) = file_layer {
        subscriber.with(file_layer).try_init().ok();
    } else {
        subscriber.try_init().ok();
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

/// Arranca el hilo vigilante que detecta ciclos de deadlock de `parking_lot`
/// y los registra con `tracing::error!`. Solo se compila con la feature
/// `deadlock-detection`; el hilo únicamente duerme y consulta el grafo de
/// espera, por lo que no toma ningún lock de la aplicación.
#[cfg(feature = "deadlock-detection")]
pub fn spawn_deadlock_watchdog() {
    if let Err(e) = std::thread::Builder::new()
        .name("audoxidy-deadlock-watchdog".into())
        .spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                for (i, threads) in parking_lot::deadlock::check_deadlock().iter().enumerate() {
                    tracing::error!("deadlock cycle #{} detected ({} threads)", i, threads.len());
                    for t in threads {
                        tracing::error!("  thread {:?}\n{:?}", t.thread_id(), t.backtrace());
                    }
                }
            }
        })
    {
        tracing::error!("No se pudo iniciar el vigilante de deadlocks: {e}");
    }
}

// ── Colector de métricas de rendimiento ──────────────────────

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
    pub fn new() -> Self {
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

/// Layer de tracing que captura métricas de rendimiento desde eventos
/// estructurados cuyo target es `audoxidy::metrics`.
struct MetricsLayer {
    metrics: &'static PerformanceMetrics,
}

impl MetricsLayer {
    fn new(metrics: &'static PerformanceMetrics) -> Self {
        Self { metrics }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for MetricsLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() != METRICS_TARGET {
            return;
        }

        let mut visitor = MetricsVisitor::default();
        event.record(&mut visitor);

        let (Some(metric), Some(value)) = (visitor.metric.as_deref(), visitor.value) else {
            return;
        };

        match metric {
            "audio_frames_processed" => {
                self.metrics
                    .audio_frames_processed
                    .fetch_add(value, Ordering::Relaxed);
            }
            "ram_usage_bytes" => {
                self.metrics.ram_usage_bytes.store(value, Ordering::Relaxed);
            }
            "audio_latency" => {
                self.metrics
                    .audio_latency_peak_us
                    .fetch_max(value, Ordering::Relaxed);
            }
            "underrun" => {
                self.metrics
                    .underrun_count
                    .fetch_add(value, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct MetricsVisitor {
    metric: Option<String>,
    value: Option<u64>,
}

impl tracing::field::Visit for MetricsVisitor {
    /// Captura el campo `metric` (`record_str` por defecto deriva en
    /// `record_debug`, por lo que hay que implementarlo explícitamente).
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "metric" {
            self.metric = Some(value.to_owned());
        }
    }

    /// Captura el valor numérico (`u64`/`u32`/`usize`).
    fn record_u64(&mut self, _field: &tracing::field::Field, value: u64) {
        self.value = Some(value);
    }

    fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    /// Layer de prueba que expone los campos capturados por `MetricsVisitor`.
    struct CaptureLayer {
        captured: Arc<Mutex<Option<(String, u64)>>>,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CaptureLayer {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if event.metadata().target() != METRICS_TARGET {
                return;
            }
            let mut visitor = MetricsVisitor::default();
            event.record(&mut visitor);
            if let (Some(metric), Some(value)) = (visitor.metric, visitor.value) {
                *self.captured.lock().unwrap() = Some((metric, value));
            }
        }
    }

    #[test]
    fn visitor_records_fields() {
        let captured = Arc::new(Mutex::new(None));
        let layer = CaptureLayer {
            captured: Arc::clone(&captured),
        };
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                target: "audoxidy::metrics",
                metric = "ram_usage_bytes",
                ram_usage_bytes = 4096u64
            );
        });

        let got = captured.lock().unwrap().clone();
        assert_eq!(got, Some(("ram_usage_bytes".to_string(), 4096)));
    }

    #[test]
    fn metrics_events_update_counters() {
        let metrics: &'static PerformanceMetrics = Box::leak(Box::new(PerformanceMetrics::new()));
        let subscriber = tracing_subscriber::registry().with(MetricsLayer::new(metrics));

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                target: "audoxidy::metrics",
                metric = "audio_frames_processed",
                frames = 480u64
            );
            tracing::warn!(
                target: "audoxidy::metrics",
                metric = "underrun",
                count = 3u64
            );
            tracing::info!(
                target: "audoxidy::metrics",
                metric = "ram_usage_bytes",
                ram_usage_bytes = 1024u64
            );
            tracing::info!(
                target: "audoxidy::metrics",
                metric = "audio_latency",
                latency_us = 500u64
            );
            tracing::info!(
                target: "audoxidy::metrics",
                metric = "audio_latency",
                latency_us = 300u64
            );
        });

        assert_eq!(metrics.audio_frames_processed.load(Ordering::Relaxed), 480);
        assert_eq!(metrics.underrun_count.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.ram_usage_bytes.load(Ordering::Relaxed), 1024);
        assert_eq!(metrics.audio_latency_peak_us.load(Ordering::Relaxed), 500);
    }

    /// Layer de prueba que observa la apertura, los registros y el cierre de
    /// spans para comprobar que la latencia registrada llega al subscriber.
    struct SpanCaptureLayer {
        opened: Arc<AtomicUsize>,
        closed: Arc<AtomicUsize>,
        elapsed_us: Arc<Mutex<Option<u64>>>,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for SpanCaptureLayer {
        fn on_new_span(
            &self,
            _attrs: &tracing::span::Attributes<'_>,
            _id: &tracing::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            self.opened.fetch_add(1, Ordering::Relaxed);
        }

        fn on_record(
            &self,
            _id: &tracing::span::Id,
            values: &tracing::span::Record<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut visitor = SpanFieldsVisitor::default();
            values.record(&mut visitor);
            if let Some(v) = visitor.elapsed_us {
                *self.elapsed_us.lock().unwrap() = Some(v);
            }
        }

        fn on_close(
            &self,
            _id: tracing::span::Id,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            self.closed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Visitante que extrae el campo `elapsed_us` de un registro de span.
    #[derive(Default)]
    struct SpanFieldsVisitor {
        elapsed_us: Option<u64>,
    }

    impl tracing::field::Visit for SpanFieldsVisitor {
        fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
            if field.name() == "elapsed_us" {
                self.elapsed_us = Some(value);
            }
        }

        fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {}
    }

    /// Un span de diagnóstico con latencia registrada debe entregar su evento
    /// de cierre al subscriber.
    #[test]
    fn span_close_events_deliver_recorded_latency() {
        let opened = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let elapsed_us = Arc::new(Mutex::new(None));
        let layer = SpanCaptureLayer {
            opened: Arc::clone(&opened),
            closed: Arc::clone(&closed),
            elapsed_us: Arc::clone(&elapsed_us),
        };
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            if tracing::enabled!(tracing::Level::DEBUG) {
                let span = tracing::debug_span!(
                    "batch_smoke",
                    frames = tracing::field::Empty,
                    channels = tracing::field::Empty,
                    elapsed_us = tracing::field::Empty
                );
                let _enter = span.enter();
                span.record("frames", 480usize);
                span.record("channels", 2usize);
                span.record("elapsed_us", 42u64);
            }
        });

        assert_eq!(opened.load(Ordering::Relaxed), 1, "el span debe abrirse");
        assert_eq!(
            closed.load(Ordering::Relaxed),
            1,
            "el evento de cierre debe entregarse"
        );
        assert_eq!(
            *elapsed_us.lock().unwrap(),
            Some(42),
            "la latencia registrada debe llegar al subscriber"
        );
    }

    /// Con el nivel por defecto (`info`) los spans de DEBUG quedan apagados y
    /// no generan eventos.
    #[test]
    fn debug_spans_stay_off_at_info_level() {
        let opened = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let layer = SpanCaptureLayer {
            opened: Arc::clone(&opened),
            closed: Arc::clone(&closed),
            elapsed_us: Arc::new(Mutex::new(None)),
        };
        let subscriber = tracing_subscriber::registry()
            .with(layer)
            .with(tracing_subscriber::filter::LevelFilter::INFO);

        tracing::subscriber::with_default(subscriber, || {
            assert!(!tracing::enabled!(tracing::Level::DEBUG));
            let span = tracing::debug_span!("batch_smoke_disabled");
            let _enter = span.enter();
        });

        assert_eq!(opened.load(Ordering::Relaxed), 0);
        assert_eq!(closed.load(Ordering::Relaxed), 0);
    }
}

/// Prueba de humo del detector de deadlocks: con el grafo de espera limpio no
/// debe reportar ciclos y el vigilante debe poder arrancar sin entrar en pánico.
#[cfg(all(test, feature = "deadlock-detection"))]
mod deadlock_tests {
    use super::spawn_deadlock_watchdog;

    #[test]
    fn deadlock_detector_smoke() {
        let cycles = parking_lot::deadlock::check_deadlock();
        assert!(
            cycles.is_empty(),
            "unexpected deadlock cycles in a single-threaded test"
        );
        spawn_deadlock_watchdog();
    }
}
