//! Tracing layer and metrics collector tests.
//!
//! A capture layer stands in for the real subscriber, so the visitor that reads
//! the structured metric fields and the layer that folds them into the atomic
//! counters are asserted without installing a global subscriber, opening a log
//! file or writing to stderr.

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
            target: METRICS_TARGET,
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
            target: METRICS_TARGET,
            metric = "audio_frames_processed",
            frames = 480u64
        );
        tracing::warn!(
            target: METRICS_TARGET,
            metric = "underrun",
            count = 3u64
        );
        tracing::info!(
            target: METRICS_TARGET,
            metric = "ram_usage_bytes",
            ram_usage_bytes = 1024u64
        );
        tracing::info!(
            target: METRICS_TARGET,
            metric = "audio_latency",
            latency_us = 500u64
        );
        tracing::info!(
            target: METRICS_TARGET,
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

    fn on_close(&self, _id: tracing::span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
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

/// Los tres destinos de enrutamiento son un contrato publicado: una directiva
/// de `RUST_LOG` los nombra, así que un valor distinto mueve cada evento a un
/// destino que ninguna directiva alcanza y el registro queda vacío sin error.
#[test]
fn tracing_targets_keep_their_published_values() {
    assert_eq!(METRICS_TARGET, "audoxidy::metrics");
    assert_eq!(PERSISTENCE_TARGET, "audoxidy::persistence");
    assert_eq!(COVERS_TARGET, "audoxidy::covers");
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
